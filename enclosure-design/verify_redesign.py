#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Check the V16 candidate's USB roots, nominal solids and assembly paths."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
import subprocess

from preview import read_off
from load_root_checks import jobs as load_root_jobs
from thin_wall_checks import jobs as wall_jobs
from print_mesh import prepare_stl
from redesign_model import ROOT, main as generate, parameters
from verify_trial import PARTS, volume


BUILD = ROOT / "build"
CASE_LIMIT_MM = (51.8, 32.1, 19.0)
EXPECTED_CHECKS = 111
SOURCES = ("redesign_model.py", "redesign_geometry.py", "verify_redesign.py", "print_mesh.py",
           "redesign_load_roots.py", "load_root_checks.py", "model.py", "board_reference.py",
           "trial_model.py", "trial_fillets.py", "trial_carriers.py")
SOURCES += ("thin_wall_checks.py", "redesign_perimeter.py")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--openscad", required=True)
    parser.add_argument("--without-front-stop", action="store_true")
    parser.add_argument("--without-usb-rounds", action="store_true")
    parser.add_argument("--without-mesh-cleanup", action="store_true")
    parser.add_argument("--without-precision-cleanup", action="store_true")
    parser.add_argument("--without-load-roots", action="store_true")
    parser.add_argument("--without-thickness-fix", action="store_true")
    parser.add_argument("--without-local-root-fix", action="store_true")
    args = parser.parse_args()
    generate()
    values = parameters()
    source_hashes = {name: hashlib.sha256((ROOT/name).read_bytes()).hexdigest() for name in SOURCES}
    scad = BUILD / "concept-v16.scad"
    scad_hash = hashlib.sha256(scad.read_bytes()).hexdigest()
    prefix = ("v16-no-floor-root-fix" if args.without_local_root_fix else
              "v16-no-thickness-fix" if args.without_thickness_fix else
              "v16-no-precision-cleanup" if args.without_precision_cleanup else
              "v16-no-load-roots" if args.without_load_roots else
              "v16-no-stop" if args.without_front_stop else "v16-no-usb-rounds" if args.without_usb_rounds
              else "v16-no-mesh-cleanup" if args.without_mesh_cleanup else "v16")
    results = []

    def run(name, part, definitions=(), empty=False, suffix="off", preview=False):
        output = BUILD / f"{prefix}-{name}.{suffix}"
        output.unlink(missing_ok=True)
        raw = output.with_name(output.stem+"-raw.stl") if suffix == "stl" else output
        command = [args.openscad, "-o", str(raw), "-D", f'part="{part}"']
        if suffix == "stl":
            command.extend(("--export-format", "binstl"))
        if args.without_front_stop:
            command.extend(("-D", "front_stop_enabled=false"))
        if args.without_usb_rounds:
            command.extend(("-D", "usb_slot_bottom_radius=0"))
        if args.without_load_roots:
            command.extend(("-D", "load_roots_enabled=false"))
        if args.without_local_root_fix:
            command.extend(("-D", "local_wall_roots=false"))
        if args.without_thickness_fix:
            for definition in ("carrier_ear_diameter=6.4", "mount_bulges=false",
                               "screws=[[28.5,-9.8],[28.5,9.8]]", "rear_stand_y=5.7",
                               "rear_stand_width=2.4", "rear_post_depth=1.6",
                               "rear_top_thickness=1", "rear_foot_half_width=7",
                               "top_pad_end_clearance=0.4", "lid_guide_width=0.8"):
                command.extend(("-D", definition))
        for definition in definitions:
            command.extend(("-D", definition))
        source = scad
        if part == "stl-roundtrip":
            source = BUILD/f"{prefix}-{name}.scad"
            stl = f"{prefix}-{name.removeprefix('roundtrip-')}.stl"
            source.write_text("// SPDX-License-Identifier: AGPL-3.0-only\n"
                              "// Copyright (C) 2026 RS-Key contributors\n"
                              f'intersection() {{ import("{stl}"); '
                              'translate([-10,-30,-10]) cube([100,60,50]); }\n')
        command.append(str(source))
        process = subprocess.run(command, capture_output=True, text=True, timeout=600,
                                 env={**os.environ, "QT_QPA_PLATFORM": "offscreen"})
        log = process.stdout+process.stderr
        prepared = None
        if suffix == "stl" and process.returncode == 0 and "ERROR:" not in log:
            try:
                prepared = prepare_stl(raw, output, part, simplify=not args.without_mesh_cleanup,
                                       precision_cleanup=not args.without_precision_cleanup)
            except ValueError as error:
                log += f"\nERROR: {error}\n"
        (BUILD/f"{prefix}-{name}-{suffix}.log").write_text(log)
        measured = volume(output) if suffix == "off" and output.is_file() else None
        if empty:
            passed = (process.returncode == 1 and "Current top level object is empty" in log
                      and "ERROR:" not in log and not output.exists())
            if process.returncode == 0 and measured is not None:
                passed = "ERROR:" not in log and measured < 1e-6
        else:
            passed = process.returncode == 0 and output.is_file() and "ERROR:" not in log
            if not preview:
                if part in PARTS or part in ("stl-roundtrip", "load-root-core"):
                    passed = passed and "Simple:        yes" in log and "Volumes:         2" in log
                elif measured is not None:
                    # Contact faces can accompany a positive-volume collision mesh.
                    passed = passed and measured > 1e-5
        return {"name": name, "passed": passed, "expected": "empty/contact faces only" if empty else
                "reference scene" if preview else "nonempty solid", "returncode": process.returncode,
                "volume_mm3": measured, **({"print_mesh": prepared} if prepared else {})}

    def batch(jobs):
        with ThreadPoolExecutor(max_workers=4) as pool:
            for result in pool.map(lambda job: run(*job), jobs):
                results.append(result)
                print(f'{"PASS" if result["passed"] else "FAIL"}: {result["name"]}', flush=True)

    def write_report(meshes=None):
        report = {"all_passed": all(row["passed"] for row in results), "checks": results,
                  "source_sha256": source_hashes, "scad_sha256": scad_hash,
                  "meshes": meshes or {}, "status": "architecture candidate; physical strength untested"}
        (BUILD/f"{prefix}-geometry-checks.json").write_text(json.dumps(report, indent=2)+"\n")
        return report

    if not args.without_mesh_cleanup:
        batch(wall_jobs())
        if not all(row["passed"] for row in results):
            write_report()
            raise SystemExit(1)
        batch((("unmodified-floor-fillets-retained", "floor-root-loss", (), True),))
        if not all(row["passed"] for row in results):
            write_report()
            raise SystemExit(1)
        batch((("base-stop-blocks-forward-0.10mm", "base-board-collision", ("collision_shift=[-0.1,0,0]",)),
               ("removed-base-stop-no-longer-blocks", "base-board-collision",
                ("collision_shift=[-0.1,0,0]", "front_stop_enabled=false"), True)))
        if not all(row["passed"] for row in results):
            write_report()
            raise SystemExit(1)
        batch((
            ("usb-left-root-has-added-material", "usb-slot-root-material", ("usb_root_side=-1",)),
            ("usb-right-root-has-added-material", "usb-slot-root-material", ("usb_root_side=1",)),
            ("usb-rounds-preserve-square-slot-core", "usb-slot-material-loss", (), True),
            ("usb-rounds-retain-air-inside-arcs", "usb-slot-root-air", (), True),
            ("usb-0.25mm-radial-clearance", "usb-clearance-collision", (), True),
            ("usb-clearance-shift-positive-control", "usb-clearance-collision", ("collision_shift=[0,0.3,0]",)),
            ("usb-R0.8-invades-reserved-clearance", "usb-clearance-collision", ("usb_slot_bottom_radius=0.8",)),
        ))
        if not all(row["passed"] for row in results):
            write_report()
            raise SystemExit(1)
        batch(load_root_jobs(values))
        if not all(row["passed"] for row in results):
            write_report()
            raise SystemExit(1)
    batch([(part, part, (), False, suffix) for part in PARTS for suffix in ("off", "stl")])
    if not all(row["passed"] for row in results):
        write_report()
        raise SystemExit(1)
    batch([(f"roundtrip-{part}", "stl-roundtrip") for part in PARTS])
    if not all(row["passed"] for row in results):
        write_report()
        raise SystemExit(1)
    meshes = {}
    for part in PARTS:
        vertices, _ = read_off(BUILD/f"{prefix}-{part}.off")
        meshes[part] = {
            "bounds": [min(v[i] for v in vertices) for i in range(3)] +
                      [max(v[i] for v in vertices) for i in range(3)],
            "volume_mm3": volume(BUILD/f"{prefix}-{part}.off"),
            "stl_sha256": hashlib.sha256((BUILD/f"{prefix}-{part}.stl").read_bytes()).hexdigest(),
            "print_mesh": next(row["print_mesh"] for row in results
                               if row["name"] == part and "print_mesh" in row),
        }
    front = min(mesh["bounds"][0] for mesh in meshes.values())
    length, width, height = CASE_LIMIT_MM
    results.append({"name": "whole-envelope-and-usb-budget", "passed":
                    abs(front) < 1e-6 and values["pcb_x"]-front+values["usb_blocking_reserve"] <= 1 and
                    all(mesh["bounds"][1] >= -width/2-1e-5 and mesh["bounds"][4] <= width/2+1e-5 and
                        mesh["bounds"][2] >= -1e-6 and mesh["bounds"][5] <= height+1e-5 and
                        mesh["bounds"][0] >= -1e-6 and mesh["bounds"][3] <= length+1e-5
                        for mesh in meshes.values()), "bounds_mm": [0,-width/2,0,length,width/2,height]})
    jobs = [(f"pair-{a}-{b}", "pair-collision", (f"pair=[{a},{b}]",), True)
            for a in range(4) for b in range(a+1,4)]
    jobs.extend((
        ("official-components-and-pcb-clear", "board-collision", (), True),
        ("14mm-stack-except-bearing-patches-clear", "stack-collision", (), True),
        ("sensor-clear", "sensor-collision", (), True),
        ("usb-clear", "usb-collision", (), True),
        ("pads-clear", "pad-collision", (), True),
        ("metal-1.2mm-reservation-clear", "metal-collision", (), True),
        ("cable-corridor-clear", "cable-collision", (), True),
        ("rear-stop-blocks-pcb-0.15mm", "board-collision", ("collision_shift=[0.15,0,0]",)),
        ("carrier-pin-blocks-rearward-0.10mm", "pair-collision", ("pair=[0,2]", "collision_shift=[0.1,0,0]")),
        ("carrier-pin-blocks-forward-0.10mm", "pair-collision", ("pair=[0,2]", "collision_shift=[-0.1,0,0]")),
        ("carrier-shoulder-blocks-drop", "pair-collision", ("pair=[0,2]", "collision_shift=[0,0,-0.1]")),
        ("cover-boss-blocks-carrier-lift", "pair-collision", ("pair=[1,2]", "collision_shift=[0,0,0.1]")),
        ("continuous-board-and-carrier-loading", "loading-sweep", (), True),
        ("continuous-cover-closing", "closing-sweep", (), True),
        ("sensor-shift-positive-control", "sensor-collision", ("collision_shift=[0,0,0.5]",)),
        ("usb-width-positive-control", "usb-collision", ("collision_shift=[0,0.5,0]",)),
        ("stack-shift-positive-control", "stack-collision", ("collision_shift=[-0.1,0,0]",)),
        ("metal-shift-positive-control", "metal-collision", ("collision_shift=[-3,0,0]",)),
    ))
    jobs.extend((f"side-loading-{distance:g}", "carrier-side-collision",
                 (f"assembly_side_offset={distance}",), True) for distance in (0,0.3,1,3,7,12,20))
    batch(jobs)
    batch([(part, part, (), False, "off", True) for part in ("board", "usb", "sensor", "pads-installed", "hardware", "footprint")])
    batch([(f"core-{part}", "load-root-core", (f"root_piece={index}",))
           for index,part in enumerate(PARTS)])
    report = write_report(meshes)
    print(json.dumps({"all_passed": report["all_passed"], "checks": len(results),
                      "failed": [row["name"] for row in results if not row["passed"]]}, indent=2))
    if not report["all_passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
