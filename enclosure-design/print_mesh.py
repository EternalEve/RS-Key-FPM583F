# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Prepare and validate printable meshes without changing nominal CAD dimensions."""

import hashlib
import struct

import manifold3d as manifold
import numpy as np
import trimesh


SIMPLIFY_TOLERANCE_MM = 0.0001
VERTEX_MERGE_TOLERANCE_MM = 0.001
SURFACE_TOLERANCE_MM = 0.0012
MAX_RELATIVE_VOLUME_CHANGE = 0.000001
PRECISION_DECIMALS = (6, 5, 4)
STL_FACET = np.dtype([("normal", "<f4", (3,)), ("vertices", "<f4", (3, 3)),
                      ("attribute", "<u2")])


def load_solid(path):
    mesh = trimesh.load_mesh(path, process=True)
    if not np.isfinite(mesh.vertices).all() or not mesh.is_watertight or not mesh.is_winding_consistent:
        raise ValueError(f"Mesh is not a finite, closed, oriented surface: {path}")
    solid = manifold.Manifold(manifold.Mesh64(
        np.asarray(mesh.vertices, dtype=np.float64), np.asarray(mesh.faces, dtype=np.uint64)))
    if solid.status() != manifold.Error.NoError or len(solid.decompose()) != 1 or solid.volume() <= 0:
        raise ValueError(f"Mesh is not one positive-volume manifold: {path}")
    return mesh, solid


def indexed_surface(triangles):
    if not np.isfinite(triangles).all():
        raise ValueError("Non-finite STL coordinates")
    vertices, indices = np.unique(triangles.reshape(-1, 3), axis=0, return_inverse=True)
    return trimesh.Trimesh(vertices, indices.reshape(-1, 3), process=False)


def check_surface(mesh):
    triangles = mesh.triangles
    normals = np.cross(triangles[:, 1]-triangles[:, 0], triangles[:, 2]-triangles[:, 0])
    if np.any(np.linalg.norm(normals, axis=1) == 0):
        raise ValueError("STL contains zero-area triangles")
    edges = mesh.faces[:, [[0, 1], [1, 2], [2, 0]]].reshape(-1, 2)
    _, inverse, counts = np.unique(np.sort(edges, axis=1), axis=0,
                                   return_inverse=True, return_counts=True)
    if np.any(counts != 2):
        raise ValueError("STL has boundary or non-manifold edges")
    if np.any(np.bincount(inverse, weights=np.where(edges[:, 0] < edges[:, 1], 1, -1))):
        raise ValueError("STL has reversed triangle winding")
    components = trimesh.graph.connected_components(mesh.face_adjacency,
                                                    nodes=np.arange(len(mesh.faces)), min_len=1)
    if len(components) != 1:
        raise ValueError("STL contains multiple surface shells")
    solid = manifold.Manifold(manifold.Mesh64(
        np.asarray(mesh.vertices), np.asarray(mesh.faces, dtype=np.uint64)))
    if solid.status() != manifold.Error.NoError or len(solid.decompose()) != 1 or solid.volume() <= 0:
        raise ValueError("STL is not one positive-volume manifold")
    return solid, normals


def validate_stl(path):
    data = path.read_bytes()
    count = struct.unpack_from("<I", data, 80)[0] if len(data) >= 84 else 0
    binary = len(data) == 84+50*count
    if binary:
        facets = np.frombuffer(data, dtype=STL_FACET, offset=84)
        triangles = facets["vertices"].astype(np.float64)
        stored_normals = facets["normal"].astype(np.float64)
    else:
        lines = data.decode("ascii").splitlines()
        triangles = np.array([[float(v) for v in line.split()[1:]] for line in lines
                              if line.strip().startswith("vertex ")]).reshape(-1, 3, 3)
        stored_normals = np.array([[float(v) for v in line.split()[2:]] for line in lines
                                  if line.strip().startswith("facet normal ")])
    if len(triangles) == 0 or stored_normals.shape != (len(triangles), 3):
        raise ValueError("STL has missing facets or normals")
    mesh = indexed_surface(triangles)
    solid, normals = check_surface(mesh)
    unit_normals = normals/np.linalg.norm(normals, axis=1)[:, None]
    if not np.isfinite(stored_normals).all() or not np.allclose(
            unit_normals, stored_normals, rtol=0, atol=0.000001):
        raise ValueError("STL facet normals disagree with serialized vertices")
    check_surface(indexed_surface(triangles.astype(np.float32).astype(np.float64)))
    for decimals in PRECISION_DECIMALS:
        # Integer cross products also catch collinear faces hidden by float cancellation.
        rounded = np.rint(triangles*10**decimals).astype(np.int64)
        cross = np.cross(rounded[:, 1]-rounded[:, 0], rounded[:, 2]-rounded[:, 0])
        if np.any(np.all(cross == 0, axis=1)):
            raise ValueError(f"STL collapses at {decimals} decimal coordinate precision")
        check_surface(indexed_surface(rounded.astype(np.float64)/10**decimals))
    return {"sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "vertices": len(mesh.vertices), "triangles": len(mesh.faces),
            "bounds_mm": mesh.bounds.tolist(), "volume_mm3": solid.volume(),
            "watertight": True, "consistent_winding": True, "components": 1,
            "zero_area_triangles": 0, "binary_stl": binary,
            "stored_normals_match": True, "float32_checked": True,
            "coordinate_decimal_checks": list(PRECISION_DECIMALS)}


def write_stl(path, vertices, faces):
    facets = np.zeros(len(faces), dtype=STL_FACET)
    facets["vertices"] = vertices[faces]
    triangles = facets["vertices"].astype(np.float64)
    normal = np.cross(triangles[:, 1]-triangles[:, 0], triangles[:, 2]-triangles[:, 0])
    length = np.linalg.norm(normal, axis=1)
    if np.any(length == 0):
        raise ValueError("Cannot export a zero-area triangle at STL float32 precision")
    facets["normal"] = normal/length[:, None]
    path.write_bytes(b"RS-Key binary STL; millimeters".ljust(80, b" ")+
                     struct.pack("<I", len(facets))+facets.tobytes())


def merge_short_edges(vertices, faces):
    parent = np.arange(len(vertices))

    def find(index):
        while parent[index] != index:
            parent[index] = parent[parent[index]]
            index = parent[index]
        return index

    edges = np.unique(np.sort(faces[:, [[0, 1], [1, 2], [2, 0]]].reshape(-1, 2), axis=1), axis=0)
    lengths = np.linalg.norm(vertices[edges[:, 0]]-vertices[edges[:, 1]], axis=1)
    for a, b in edges[lengths < VERTEX_MERGE_TOLERANCE_MM]:
        a, b = find(a), find(b)
        parent[max(a, b)] = min(a, b)
    parent = np.array([find(i) for i in range(len(vertices))])
    displacement = float(np.linalg.norm(vertices-vertices[parent], axis=1).max())
    if displacement > VERTEX_MERGE_TOLERANCE_MM:
        raise ValueError("Short-edge cluster exceeded the vertex movement budget")
    faces = parent[faces]
    faces = faces[(faces[:, 0] != faces[:, 1]) & (faces[:, 1] != faces[:, 2]) &
                  (faces[:, 0] != faces[:, 2])]
    return faces, displacement


def prepare_stl(source, destination, part, simplify=True, precision_cleanup=True):
    original_mesh, original = load_solid(source)
    solid = original.simplify(SIMPLIFY_TOLERANCE_MM) if simplify else original
    mesh = solid.to_mesh64()
    vertices = np.asarray(mesh.vert_properties)[:, :3].copy()
    faces = np.asarray(mesh.tri_verts)
    lost, added = (original-solid).volume(), (solid-original).volume()
    if (lost+added)/original.volume() > MAX_RELATIVE_VOLUME_CHANGE:
        raise ValueError("STL preparation changed too much material")
    if not set(map(tuple, vertices)) <= set(map(tuple, original_mesh.vertices)):
        raise ValueError("STL preparation introduced new vertex positions")
    bounds = np.array([vertices.min(axis=0), vertices.max(axis=0)])
    if np.max(np.abs(bounds-original_mesh.bounds)) > SURFACE_TOLERANCE_MM:
        raise ValueError("STL preparation moved the envelope")
    rotation = np.diag([1, -1, -1]) if part == "lid" else np.eye(3)
    vertices = vertices @ rotation.T
    origin = vertices.min(axis=0)
    vertices -= origin
    displacement = 0
    if simplify and precision_cleanup:
        faces, displacement = merge_short_edges(vertices, faces)
    inverse = np.eye(4)
    inverse[:3, :3] = rotation.T
    inverse[:3, 3] = rotation.T @ origin
    write_stl(destination, vertices, faces)
    report = validate_stl(destination)
    actual_mesh = indexed_surface(vertices[faces].astype(np.float32).astype(np.float64))
    # The lid's translated origin is in print axes, just like the serialized vertices.
    actual = manifold.Manifold(manifold.Mesh64(
        (actual_mesh.vertices+origin) @ rotation.T,
        np.asarray(actual_mesh.faces, dtype=np.uint64)))
    lost, added = (original-actual).volume(), (actual-original).volume()
    if (lost+added)/original.volume() > MAX_RELATIVE_VOLUME_CHANGE:
        raise ValueError("Serialized STL changed too much material")
    if np.max(np.abs(np.asarray(actual.bounding_box())-original.bounding_box())) > SURFACE_TOLERANCE_MM:
        raise ValueError("Serialized STL moved the envelope")
    report.update({"raw_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                   "surface_tolerance_mm": SURFACE_TOLERANCE_MM if simplify else 0,
                   "simplify_tolerance_mm": SIMPLIFY_TOLERANCE_MM if simplify else 0,
                   "vertex_merge_tolerance_mm": VERTEX_MERGE_TOLERANCE_MM if precision_cleanup else 0,
                   "maximum_merged_vertex_displacement_mm": displacement,
                   "lost_material_mm3": lost, "added_material_mm3": added,
                   "relative_material_change": (lost+added)/original.volume(),
                   "assembly_from_print": inverse.tolist()})
    return report
