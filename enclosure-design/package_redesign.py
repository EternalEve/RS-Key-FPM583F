#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Package the checked V16 print meshes, plate, pads and assembly instructions."""

import hashlib
import json
import re
import subprocess
import xml.etree.ElementTree as ET
import zipfile

import numpy as np
import trimesh
from trimesh.exchange.threemf import export_3MF

from pad_templates import write_template
from load_review import compare_previous, legacy_mesh_controls
from load_root_checks import root_probes
from print_mesh import indexed_surface, validate_stl
from redesign_model import ROOT, parameters
from verify_trial import PARTS
from verify_redesign import EXPECTED_CHECKS
from thin_wall_checks import jobs as wall_jobs


BUILD = ROOT / "build"
PLACEMENTS = ((5, 5, 0), (65, 5, 0), (5, 45, 0), (45, 45, 0))


def check_slicer_info(log, expected_facets):
    ordinary = {"size_x", "size_y", "size_z", "min_x", "min_y", "min_z", "max_x",
                "max_y", "max_z", "number_of_facets", "number_of_parts", "volume"}
    repairs = {key: float(value) for key, value in
               re.findall(r"^([a-z_]+)\s*=\s*([-+\d.eE]+)\s*$", log, re.MULTILINE)
               if key not in ordinary and float(value) != 0}
    counts = re.findall(r"number_of_parts\s*=\s*(\d+)", log)
    facets = [int(value) for value in re.findall(r"number_of_facets\s*=\s*(\d+)", log)]
    if ("error" in log.lower() or log.count("manifold = yes") != len(expected_facets) or
            counts != ["1"]*len(expected_facets) or repairs or
            sorted(facets) != sorted(expected_facets)):
        raise ValueError(f"Slicer repaired or changed the mesh: {repairs}")


def make_plate():
    scene = trimesh.Scene()
    for part, position in zip(PARTS, PLACEMENTS):
        mesh = indexed_surface(trimesh.load_mesh(BUILD/f"v16-{part}.stl", process=False).triangles)
        mesh.units = "mm"
        transform = np.eye(4)
        transform[:3, 3] = position
        scene.add_geometry(mesh, geom_name=part, node_name=part, transform=transform)
    plate = BUILD/"v16-four-parts.3mf"
    plate.write_bytes(export_3MF(scene))
    with zipfile.ZipFile(plate) as archive:
        if archive.testzip() is not None:
            raise ValueError("Corrupt 3MF archive")
        model = ET.fromstring(archive.read("3D/3dmodel.model"))
        namespace = {"m": "http://schemas.microsoft.com/3dmanufacturing/core/2015/02"}
        if model.get("unit") != "millimeter" or len(model.findall("m:build/m:item", namespace)) != 4:
            raise ValueError("3MF must contain four objects in millimeters")
    loaded = trimesh.load_scene(plate, process=False)
    if set(loaded.geometry) != set(PARTS) or not np.allclose(loaded.bounds, scene.bounds, atol=1e-8, rtol=0):
        raise ValueError("3MF did not reload as four parts")
    for name, mesh in loaded.geometry.items():
        if not mesh.is_watertight or not mesh.is_winding_consistent:
            raise ValueError(f"Invalid mesh in 3MF: {name}")
        surfaces = lambda geometry: {tuple(sorted(map(tuple, geometry.vertices[face])))
                                     for face in geometry.faces}
        if surfaces(mesh) != surfaces(scene.geometry[name]):
            raise ValueError(f"3MF changed the STL surface: {name}")
    for path in (plate, *(BUILD/f"v16-{part}.stl" for part in PARTS)):
        process = subprocess.run(["prusa-slicer", "--info", str(path)], capture_output=True,
                                 text=True, timeout=120)
        log = process.stdout+process.stderr
        (BUILD/f"{path.stem}-slicer.log").write_text(log)
        expected_facets = ([len(scene.geometry[part].faces) for part in PARTS] if path == plate
                           else [validate_stl(path)["triangles"]])
        if process.returncode != 0:
            raise ValueError(f"Slicer import failed: {path}")
        check_slicer_info(log, expected_facets)
    return plate


def main():
    report = json.loads((BUILD/"v16-geometry-checks.json").read_text())
    if not report["all_passed"] or len(report["checks"]) != EXPECTED_CHECKS:
        raise ValueError("The complete V16 geometry and print checks must pass")
    for name, expected in report["source_sha256"].items():
        if hashlib.sha256((ROOT/name).read_bytes()).hexdigest() != expected:
            raise ValueError(f"Source changed after verification: {name}")
    scad = BUILD/"concept-v16.scad"
    if hashlib.sha256(scad.read_bytes()).hexdigest() != report["scad_sha256"]:
        raise ValueError("CAD changed after verification")
    gate = BUILD/"v16-project-check.log"
    if not gate.read_text().rstrip().endswith("ALL CHECKS PASSED"):
        raise ValueError("The full project gate must pass before packaging")
    for prefix, failures in (
        ("v16-no-stop", {"base-stop-blocks-forward-0.10mm"}),
        ("v16-no-usb-rounds", {"usb-left-root-has-added-material", "usb-right-root-has-added-material"}),
        ("v16-no-mesh-cleanup", set(PARTS)),
        ("v16-no-precision-cleanup", set(PARTS)),
        ("v16-no-thickness-fix", {job[0] for job in wall_jobs()}),
        ("v16-no-floor-root-fix", {"unmodified-floor-fillets-retained"}),
        ("v16-no-load-roots", {*(f"load-roots-add-to-{part}" for part in PARTS),
                               *(f"root-{name}-has-material" for name,_,_ in root_probes(parameters())),
                               "front-corner-R2.0-invades-loading-reserve"}),
    ):
        control = json.loads((BUILD/f"{prefix}-geometry-checks.json").read_text())
        actual = {row["name"] for row in control["checks"] if not row["passed"]}
        if (actual != failures or control["source_sha256"] != report["source_sha256"] or
                control["scad_sha256"] != report["scad_sha256"]):
            raise ValueError(f"Missing or stale failure control: {prefix}")
    for part, metadata in report["meshes"].items():
        mesh = validate_stl(BUILD/f"v16-{part}.stl")
        if mesh["sha256"] != metadata["stl_sha256"] or abs(mesh["bounds_mm"][0][2]) > 1e-8:
            raise ValueError(f"Changed mesh or floating print origin: {part}")
    proof = compare_previous(report)
    repair_proof = legacy_mesh_controls()
    pads = BUILD/"v16-silicone-pads-1to1.svg"
    write_template(parameters(), "V16", pads)
    plate = make_plate()
    with zipfile.ZipFile(BUILD/"v15-source-preserved.zip") as archive:
        old_log = archive.read("build/v15-base-slicer.log").decode()
    try:
        check_slicer_info(old_log, [10188])
    except ValueError as error:
        if "'degenerate_facets': 20.0" not in str(error) or "'facets_removed': 20.0" not in str(error):
            raise ValueError("Legacy slicer control failed for the wrong reason") from error
    else:
        raise ValueError("The slicer gate accepted the V15 auto-repair log")
    members = [(ROOT/"concept-v16.md", "PRINT-README.md"),
               (ROOT/"manufacturing-review-v16.md", "manufacturing-review-v16.md"),
               (scad, "CAD/concept-v16.scad"),
               (BUILD/"v16-parameters.json", "CAD/v16-parameters.json"),
               (plate, "PRINT/v16-four-parts.3mf"),
               (pads, "PADS/v16-silicone-pads-1to1.svg"),
               (BUILD/"concept-v16-review.png", "preview.png"),
               (BUILD/"concept-v16-usb-detail.png", "usb-detail.png"),
               (BUILD/"concept-v16-load-roots.png", "load-roots.png"),
               (BUILD/"concept-v16-thin-walls.png", "thin-walls.png"),
               (BUILD/"v16-geometry-checks.json", "checks/geometry-and-meshes.json"),
               (proof, "checks/v15-r1-design-comparison.json"),
               (repair_proof, "checks/legacy-mesh-controls.json"),
               (gate, "checks/project-check.log")]
    for part in PARTS:
        members.append((BUILD/f"v16-{part}.stl", f"PRINT/v16-{part}.stl"))
        members.append((BUILD/f"v16-roundtrip-{part}-off.log", f"checks/roundtrip-{part}.log"))
        members.append((BUILD/f"v16-{part}-slicer.log", f"checks/slicer-{part}.log"))
    members.append((BUILD/"v16-four-parts-slicer.log", "checks/slicer-3mf.log"))
    for prefix in ("v16-no-stop", "v16-no-usb-rounds", "v16-no-mesh-cleanup",
                   "v16-no-precision-cleanup", "v16-no-load-roots", "v16-no-thickness-fix",
                   "v16-no-floor-root-fix"):
        members.append((BUILD/f"{prefix}-geometry-checks.json", f"checks/{prefix}.json"))
    members.append((BUILD/"v16-no-mesh-cleanup-base-stl.log", "checks/no-mesh-cleanup-base.log"))
    for part in PARTS:
        members.append((BUILD/f"v16-no-precision-cleanup-{part}-stl.log",
                        f"checks/no-precision-cleanup-{part}.log"))
    manifest = {name: hashlib.sha256(path.read_bytes()).hexdigest() for path, name in members}
    destination = BUILD/"RS-Key-enclosure-V16-complete-print-fit-prototype.zip"
    with zipfile.ZipFile(destination, "w", zipfile.ZIP_DEFLATED) as archive:
        for path, name in members:
            archive.write(path, name)
        archive.writestr("checks/sha256.json", json.dumps(manifest, indent=2)+"\n")
    with zipfile.ZipFile(destination) as archive:
        if archive.testzip() is not None:
            raise ValueError("Corrupt print package")
        for name, expected in manifest.items():
            if hashlib.sha256(archive.read(name)).hexdigest() != expected:
                raise ValueError(f"Packaged file hash mismatch: {name}")
    print(destination)
    print(f"{len(members)} files; {destination.stat().st_size} bytes; CRC and SHA-256 passed")


if __name__ == "__main__":
    main()
