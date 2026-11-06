#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Package only verified V11 fit parts, instructions and reproducible CAD inputs."""

import hashlib
import json
import zipfile

from trial_model import ROOT
from verify_trial import PARTS
from pad_templates import main as generate_pads


def main():
    build = ROOT / "build"
    report = json.loads((build / "v11-geometry-checks.json").read_text())
    if not report["all_passed"]:
        raise RuntimeError("Current geometry checks have not passed")
    for name, expected in report["source_sha256"].items():
        if hashlib.sha256((ROOT / name).read_bytes()).hexdigest() != expected:
            raise RuntimeError(f"Source changed after geometry verification: {name}")
    if hashlib.sha256((build / "concept-v11.scad").read_bytes()).hexdigest() != report["scad_sha256"]:
        raise RuntimeError("Generated CAD changed after geometry verification")
    gate = build / "v11-project-check.log"
    if not gate.read_text().rstrip().endswith("ALL CHECKS PASSED"):
        raise RuntimeError("This revision's full project gate has not passed")
    generate_pads()
    members = [(ROOT / "concept-v11.md", "PRINT-README.md"),
               (ROOT / "concept-v11.md", "concept-v11.md"),
               (ROOT / "concept-v10.md", "concept-v10.md"),
               (ROOT / "concept-v9.md", "concept-v9.md"),
               (ROOT / "concept-v8.md", "concept-v8.md"),
               (ROOT / "concept-v7.md", "concept-v7.md"),
               (ROOT / "concept-v6.md", "concept-v6.md"),
               (build / "concept-v11.scad", "CAD/concept-v11.scad"),
               (build / "v11-parameters.json", "CAD/v11-parameters.json"),
               (build / "v11-silicone-pads-1to1.svg", "PADS/v11-silicone-pads-1to1.svg"),
               (build / "concept-v11-review.png", "preview.png"),
               (build / "concept-v11-front-detail.png", "front-detail.png"),
               (build / "concept-v11-key-detail.png", "key-detail.png"),
               (build / "concept-v11-carrier-detail.png", "carrier-detail.png"),
               (build / "v11-geometry-checks.json", "checks/geometry.json"),
               (gate, "checks/project-check.log")]
    for part in PARTS:
        stl = build / f"v11-{part}.stl"
        expected = report["meshes"][part]["stl_sha256"]
        if hashlib.sha256(stl.read_bytes()).hexdigest() != expected:
            raise RuntimeError(f"Mesh changed after verification: {stl}")
        members.append((stl, f"STL/{stl.name}"))
    manifest = {name: hashlib.sha256(path.read_bytes()).hexdigest() for path, name in members}
    destination = build / "RS-Key-enclosure-V11-reinforced-carriers-fit-prototype.zip"
    with zipfile.ZipFile(destination, "w", zipfile.ZIP_DEFLATED) as archive:
        for path, name in members:
            archive.write(path, name)
        archive.writestr("checks/sha256.json", json.dumps(manifest, indent=2)+"\n")
    print(destination)


if __name__ == "__main__":
    main()
