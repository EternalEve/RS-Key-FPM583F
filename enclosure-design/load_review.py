# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Bound V16 design changes and retain the legacy mesh failure controls."""

import hashlib
import json
import zipfile

import manifold3d as manifold
import numpy as np

from print_mesh import load_solid, validate_stl, MAX_RELATIVE_VOLUME_CHANGE
from redesign_model import ROOT, parameters
from verify_trial import PARTS


BUILD = ROOT / "build"


def assembly_solid(path, metadata):
    mesh, _ = load_solid(path)
    matrix = np.asarray(metadata["print_mesh"]["assembly_from_print"])
    vertices = mesh.vertices @ matrix[:3,:3].T+matrix[:3,3]
    solid = manifold.Manifold(manifold.Mesh64(vertices, np.asarray(mesh.faces,dtype=np.uint64)))
    if solid.status() != manifold.Error.NoError or len(solid.decompose()) != 1:
        raise ValueError(f"Assembly transform failed: {path}")
    return solid


def compare_previous(report):
    with zipfile.ZipFile(BUILD/"v15-r1-source-preserved.zip") as archive:
        previous = json.loads(archive.read("build/v15-r1-geometry-checks.json"))
        old_parameters = json.loads(archive.read("build/v15-r1-parameters.json"))
    values = parameters()
    allowed = {"screws", "rear_stand_y", "rear_stand_width", "lid_guide_width"}
    changed = {name for name,value in old_parameters.items() if values[name] != value}
    if changed != allowed:
        raise ValueError(f"Unexpected changes to existing design parameters: {changed}")
    proof = {"changed_existing_parameters": sorted(changed),
             "outside_change_relative_tolerance": MAX_RELATIVE_VOLUME_CHANGE,"parts": {}}
    radius = values["carrier_ear_diameter"]/2+values["fit_clearance"]+values["mount_shell_wall"]
    x = values["screws"][0][0]
    inner = min(abs(s[1]) for s in old_parameters["screws"])-values["boss_diameter"]/2-values["boss_root_radius"]-values["wall"]
    regions = [(x-radius-values["wall"],inner,-1,x+radius+values["wall"],18,21),
               (x-radius-values["wall"],-18,-1,x+radius+values["wall"],-inner,21)]
    for part in PARTS:
        old_path, path = BUILD/f"v15-r1-{part}.stl", BUILD/f"v16-{part}.stl"
        old_metadata, metadata = previous["meshes"][part], report["meshes"][part]
        for candidate, data in ((old_path,old_metadata),(path,metadata)):
            if hashlib.sha256(candidate.read_bytes()).hexdigest() != data["stl_sha256"]:
                raise ValueError(f"Changed comparison mesh: {candidate}")
        old, new = assembly_solid(old_path,old_metadata), assembly_solid(path,metadata)
        removed, inserted = old-new, new-old
        boxes = regions if part in ("base","lid") else [
            (values["pcb_x"]+values["pcb_length"]-values["rear_tab_depth"]-0.01,-18,-1,35.5,18,21)]
        if part == "lid":
            boxes = [*boxes,(3.9,10.8,16.3,23.1,12.2,17.5),(3.9,-12.2,16.3,23.1,-10.8,17.5)]
        mask = manifold.Manifold()
        for box in boxes:
            lower, upper = np.array(box[:3]), np.array(box[3:])
            mask += manifold.Manifold.cube(tuple(upper-lower)).translate(tuple(lower))
        outside_removed, outside_inserted = removed-mask, inserted-mask
        if any(solid.status() != manifold.Error.NoError
               for solid in (removed,inserted,mask,outside_removed,outside_inserted)):
            raise ValueError(f"Material comparison boolean failed: {part}")
        lost, added = removed.volume(), inserted.volume()
        outside = outside_removed.volume()+outside_inserted.volume()
        if outside/old.volume() > MAX_RELATIVE_VOLUME_CHANGE or new.volume() <= old.volume():
            raise ValueError(f"Unplanned material change outside the V16 mounting/tail regions: {part}")
        proof["parts"][part] = {"v15_r1_sha256": old_metadata["stl_sha256"],
                                 "v16_sha256": metadata["stl_sha256"],
                                 "v15_r1_volume_mm3": old.volume(), "v16_volume_mm3": new.volume(),
                                 "lost_material_mm3": lost,"added_material_mm3": added,
                                 "allowed_change_boxes_mm": boxes,"outside_change_mm3": outside}
    destination = BUILD/"v16-design-comparison.json"
    destination.write_text(json.dumps(proof,indent=2)+"\n")
    return destination


def legacy_mesh_controls():
    with zipfile.ZipFile(BUILD/"v15-source-preserved.zip") as archive:
        previous = json.loads(archive.read("build/v15-geometry-checks.json"))
    proof = {"parts": {}}
    for part in PARTS:
        old_path = BUILD/f"v15-{part}.stl"
        old_metadata = previous["meshes"][part]
        if hashlib.sha256(old_path.read_bytes()).hexdigest() != old_metadata["stl_sha256"]:
            raise ValueError(f"Changed legacy reproduction mesh: {part}")
        try:
            validate_stl(old_path)
        except ValueError as error:
            rejected = str(error)
        else:
            raise ValueError(f"The precision guard accepted the old V15 mesh: {part}")
        proof["parts"][part] = {"v15_sha256": old_metadata["stl_sha256"],
                                 "legacy_rejection": rejected}
    destination = BUILD/"v16-legacy-mesh-checks.json"
    destination.write_text(json.dumps(proof, indent=2)+"\n")
    return destination
