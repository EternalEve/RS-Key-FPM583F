#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Export V11 parts and check retention, mating, isolation and assembly travel."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess

from preview import read_off
from trial_model import ROOT, main as generate, parameters
from trial_key_checks import key_checks
from trial_carrier_checks import carrier_checks


BUILD = ROOT / "build"
PARTS = ("base", "lid", "carrier-left", "carrier-right")


def volume(path):
    _, faces = read_off(path)
    total = 0.0
    for face in faces:
        a = face[0]
        for b, c in zip(face[1:], face[2:]):
            total += (a[0]*(b[1]*c[2]-b[2]*c[1]) +
                      a[1]*(b[2]*c[0]-b[0]*c[2]) +
                      a[2]*(b[0]*c[1]-b[1]*c[0]))/6
    return abs(total)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--openscad", required=True)
    args = parser.parse_args()
    generate()
    values = parameters()
    sources = ("model.py", "board_reference.py", "trial_model.py", "trial_geometry.py", "trial_fillets.py",
               "trial_carriers.py", "trial_carrier_checks.py", "trial_rib_reference.py", "trial_front_keys.py", "trial_key_checks.py",
               "verify_trial.py", "pad_templates.py", "trial_preview.py", "package_trial.py",
               "concept-v11.md", "concept-v10.md", "concept-v9.md", "concept-v8.md", "concept-v7.md", "concept-v6.md", "dimensions.md", "README.md")
    source_hashes = {name: hashlib.sha256((ROOT/name).read_bytes()).hexdigest()
                     for name in sources}
    scad_hash = hashlib.sha256((BUILD / "concept-v11.scad").read_bytes()).hexdigest()
    results = []

    def run(name, part, definitions=(), empty=False, suffix="off", preview=False, rejection=None):
        output = BUILD / f"v11-{name}.{suffix}"
        output.unlink(missing_ok=True)
        command = [args.openscad, "-o", str(output), "-D", f'part="{part}"']
        for definition in definitions:
            command.extend(("-D", definition))
        command.append(str(BUILD / "concept-v11.scad"))
        process = subprocess.run(command, capture_output=True, text=True, timeout=600,
                                 env={**os.environ, "QT_QPA_PLATFORM": "offscreen"})
        log = process.stdout + process.stderr
        (BUILD / f"v11-{name}-{suffix}.log").write_text(log)
        if rejection:
            passed = (process.returncode != 0 and rejection in log
                      and "ERROR: Assertion" in log and not output.exists())
        elif empty:
            passed = (process.returncode == 1 and "Current top level object is empty" in log
                      and "ERROR:" not in log and not output.exists())
            if process.returncode == 0 and suffix == "off" and output.is_file():
                # Intended mating planes can survive CGAL as zero-thickness faces.
                passed = "ERROR:" not in log and volume(output) < 1e-6
        else:
            passed = (process.returncode == 0 and output.is_file()
                      and "ERROR:" not in log and "Simple:        yes" in log)
            if preview:
                passed = process.returncode == 0 and output.is_file() and "ERROR:" not in log
            elif part in PARTS:
                passed = passed and "Volumes:         2" in log
            elif suffix == "off":
                passed = (process.returncode == 0 and output.is_file()
                          and "ERROR:" not in log
                          and volume(output) > (1e-8 if part == "root-probe" else 1e-3))
        return {"name": name, "format": suffix, "passed": passed,
                "expected": f"assertion rejected: {rejection}" if rejection else
                            "no solid intersection (contact faces allowed)" if empty
                            else "preview scene only" if preview else "simple nonempty mesh",
                "returncode": process.returncode}

    def export(part):
        return [run(part, part), run(part, part, suffix="stl")]

    for checks in (carrier_checks(), key_checks()):
        with ThreadPoolExecutor(max_workers=4) as pool:
            results.extend(pool.map(lambda job: run(*job), checks))
        if not all(item["passed"] for item in results):
            failed = {"all_passed": False, "checks": results}
            (BUILD / "key-regression-failure.json").write_text(json.dumps(failed, indent=2)+"\n")
            print(json.dumps(failed, indent=2))
            raise SystemExit(1)

    core_jobs = []
    for kind, regression in (("upper", "front_upper_root_rise=1.5"),
                             ("outer", "front_rib_bottom=13.0")):
        definitions = (f'web_kind="{kind}"',)
        core_jobs.extend(((f"{kind}-rib-core-preserved", "web-core-loss", definitions, True),
                          (f"{kind}-rib-v8-loss-positive-control", "web-core-loss",
                           definitions + (regression,))))
    with ThreadPoolExecutor(max_workers=4) as pool:
        results.extend(pool.map(lambda job: run(*job), core_jobs))
    if not all(item["passed"] for item in results):
        failed = {"all_passed": False, "checks": results}
        (BUILD / "rib-core-regression-failure.json").write_text(json.dumps(failed, indent=2)+"\n")
        print(json.dumps(failed, indent=2))
        raise SystemExit(1)

    with ThreadPoolExecutor(max_workers=4) as pool:
        for exported in pool.map(export, PARTS):
            results.extend(exported)
    if not all(item["passed"] for item in results):
        raise RuntimeError(f"Part export failed: {results}")

    meshes = {}
    for part in PARTS:
        vertices, _ = read_off(BUILD / f"v11-{part}.off")
        meshes[part] = {
            "bounds": [min(v[i] for v in vertices) for i in range(3)] +
                      [max(v[i] for v in vertices) for i in range(3)],
            "stl_sha256": hashlib.sha256((BUILD / f"v11-{part}.stl").read_bytes()).hexdigest(),
        }
    front = min(mesh["bounds"][0] for mesh in meshes.values())
    blocking = values["pcb_x"] - front
    results.append({"name": "all-parts-usb-blocking", "passed":
                    abs(front) < 1e-6 and blocking + values["usb_blocking_reserve"] <= 1.0,
                    "front_x": front, "nominal_blocking": blocking,
                    "combined_reserve": values["usb_blocking_reserve"]})
    pocket_front = (values["pcb_x"]+values["front_stand_depth"]-
                    values["front_tab_root_radius"]-values["fit_clearance"])
    pocket_side = max(values["front_stand_inner"]+values["carrier_side_width"],
                      values["frame_half_width"])+values["fit_clearance"]
    results.append({"name": "front-corner-skin-budget", "passed":
                    pocket_front >= values["pcb_x"]-1e-6 and
                    pocket_side <= values["front_width"]/2-values["front_radius"]+1e-6,
                    "pocket_front_x": pocket_front, "front_corner_straight_half_width":
                    values["front_width"]/2-values["front_radius"],
                    "pocket_outer_y": pocket_side,
                    "limit": "Nominal front pocket geometry, not manufacturing tolerance"})
    half_width = max(values["front_width"],values["case_width"])/2
    envelope = [0,-half_width,0,values["sensor_x"]+values["case_width"]/2,
                half_width,values["case_height"]]
    results.append({"name": "all-parts-within-case-envelope", "passed":
                    all(all(mesh["bounds"][i] >= envelope[i]-1e-6 and
                            mesh["bounds"][i+3] <= envelope[i+3]+1e-6 for i in range(3))
                        for mesh in meshes.values()), "bounds_limit": envelope})
    if not all(item["passed"] for item in results):
        raise RuntimeError(f"Part dimensional checks failed: {results[-3:]}")

    jobs = [(f"pair-{a}-{b}", "pair-collision", (f"pair=[{a},{b}]",), True)
            for a in range(len(PARTS)) for b in range(a+1, len(PARTS))]
    jobs.extend((
        ("lower-stack-clear", "lower-stack-collision", (), True),
        ("full-14mm-stack-except-bearings-clear", "full-stack-collision", (), True),
        ("sensor-clear", "sensor-collision", (), True),
        ("usb-reported-root-clear", "usb-collision", (), True),
        ("metal-reservation-clear", "metal-collision", (), True),
        ("cable-corridor-clear", "cable-collision", (), True),
        ("compressed-pads-plastic-clear", "pad-plastic-collision", (), True),
        ("front-guide-allows-0.02mm", "pair-collision",
         ("pair=[0,2]", "pair_offset=[0.02,0,0]"), True),
        ("front-guide-blocks-0.20mm", "pair-collision",
         ("pair=[0,2]", "pair_offset=[0.20,0,0]"), False),
        ("carrier-guide-blocks-forward-0.10mm", "pair-collision",
         ("pair=[0,2]", "pair_offset=[-0.10,0,0]"), False),
        ("carrier-guide-blocks-rearward-0.10mm", "pair-collision",
         ("pair=[0,2]", "pair_offset=[0.10,0,0]"), False),
        ("front-shoulder-blocks-drop", "pair-collision",
         ("pair=[0,2]", "pair_offset=[0,0,-0.10]"), False),
        ("lid-boss-blocks-carrier-lift", "pair-collision",
         ("pair=[1,2]", "pair_offset=[0,0,0.10]"), False),
        ("integral-front-blocks-pcb-extraction-motion", "board-collision",
         ("collision_shift=[-0.10,0,0]",), False),
        ("pcb-hard-stop-blocks-0.15mm", "board-collision",
         ("collision_shift=[0.15,0,0]",), False),
        ("v5-axial-play-regression-blocked", "retention-regression", (), False),
        ("continuous-loading-sweep", "loading-sweep", (), True),
        ("continuous-lid-closing-sweep", "lid-closing-sweep", (), True),
        ("carrier-seating-positive-control", "carrier-side-loading-collision",
         ("collision_shift=[0,0,0.5]",), False),
        ("front-rib-material-section", "front-rib-section", (), False),
        ("missing-front-rib-section-empty", "front-rib-section",
         ("front_rib_length=0.05",), True),
        ("front-bearing-rib-material-section", "front-bearing-rib-section", (), False),
        ("missing-front-bearing-rib-section-empty", "front-bearing-rib-section",
         ("front_bearing_rib_length=front_tab_depth",), True),
        ("stack-front-positive-control", "lower-stack-collision",
         ("collision_shift=[-3,0,0]",), False),
        ("sensor-lip-positive-control", "sensor-collision",
         ("collision_shift=[0,0,0.5]",), False),
        ("board-roof-positive-control", "board-collision",
         ("collision_shift=[0,0,4]",), False),
        ("metal-board-positive-control", "metal-collision",
         ("collision_shift=[-4,0,0]",), False),
        ("full-stack-side-positive-control", "full-stack-collision",
         ("collision_shift=[0,1,0]",), False),
        ("usb-height-positive-control", "usb-collision",
         ("collision_shift=[0,0,0.5]",), False),
    ))
    dx = values["sensor_x"]-values["front_radius"]
    dy = -(values["front_width"]/2-values["front_radius"])
    normal_angle = math.atan2(dy,dx)+math.acos(
        (values["front_radius"]-values["case_width"]/2)/math.hypot(dx,dy))
    normal = (math.cos(normal_angle),math.sin(normal_angle))
    support = (normal[0]*values["front_radius"]-normal[1]*dy+values["front_radius"])
    inner_side = (support-values["wall"]-10*normal[0])/normal[1]

    def shell_probe(inset, z):
        return (10-inset*normal[0],inner_side-inset*normal[1],z)

    root_probes = (
        ("outer-rib", 1, (values["pcb_x"]+0.15, values["front_rib_inner"]-0.15, 14.4),
         (values["pcb_x"]+0.60, values["front_rib_inner"]-0.70, 14.4), "front_rib_radius"),
        ("upper-rib", 1, (values["pcb_x"]+0.08, values["front_tab_y"]-0.58, 15.8),
         (values["pcb_x"]+0.30, values["front_tab_y"]-0.80, 15.8), "front_upper_radius"),
        ("front-roof", 1, (values["pcb_x"]+0.12, 0, 17.28),
         (values["pcb_x"]+0.45, 0, 16.95), "front_roof_radius"),
        ("lid-boss", 1, (28.5, 6.45, 17.25), (28.5, 6.0, 16.8), "boss_root_radius"),
        ("base-boss", 0, (25.15, 9.8, 1.75), (24.7, 9.8, 2.2), "boss_root_radius"),
        ("front-carrier", 2, (1.6, -10.1, 9.8), (1.6, -9.75, 9.45), "carrier_root_radius"),
        ("carrier-ear", 2, (25.2, -10.2, 6.9), (25.55, -10.2, 6.55), "carrier_root_radius"),
        ("lid-perimeter", 1, shell_probe(0.1,17.3), shell_probe(0.45,16.95), "shell_root_radius"),
        ("base-perimeter", 0, shell_probe(0.1,1.7), shell_probe(0.45,2.05), "shell_root_radius"),
    )
    for name, piece, material, air, radius in root_probes:
        common = (f"probe_piece={piece}",)
        solid = common + (f"probe_point={json.dumps(material)}",)
        jobs.extend(((f"{name}-root-material", "root-probe", solid, False),
                     (f"{name}-root-arc-air", "root-probe",
                      common + (f"probe_point={json.dumps(air)}",), True),
                     (f"{name}-root-missing-control", "root-probe", solid + (f"{radius}=0",), True)))
    # The downward loading path matters because a second PCB blocks fixed ledges.
    jobs.extend((f"loading-{lift:g}", "loading-collision", (f"assembly_lift={lift}",), True)
                for lift in (0, 0.3, 1, 3, 7, 12, 20))
    jobs.extend((f"carrier-side-loading-{offset:g}", "carrier-side-loading-collision",
                 (f"assembly_side_offset={offset}",), True)
                for offset in (0, 0.3, 1, 3, 7, 12, 20))
    with ThreadPoolExecutor(max_workers=4) as pool:
        results.extend(pool.map(lambda job: run(*job), jobs))
    results.append(run("official-components-and-nominal-pcb-clear", "board-collision", empty=True))
    results.append(run("reject-usb-blocking-over-limit", "base", ("pcb_x=1.2",),
                       rejection="USB axial blocking exceeds the 1 mm limit including reserve"))
    results.append(run("reject-no-pad-preload", "base", ("pad_compressed_thickness=0.46",),
                       rejection="pad_min_thickness > pad_compressed_thickness"))
    results.append(run("reject-roof-fillet-in-usb-clearance", "lid", ("front_roof_radius=0.8",),
                       rejection="Front roof fillet enters USB clearance"))
    results.append(run("reject-outer-fillet-in-pcb-clearance", "lid", ("front_rib_radius=1.2",),
                       rejection="Outer rib fillet enters PCB clearance"))
    results.append(run("reject-narrow-front-corner-skin", "base", ("front_width=26",),
                       rejection="Front corner relief requires a wider nose"))
    results.append(run("reject-excess-key-pull-gap", "base", ("front_key_pull_gap=0.15",),
                       rejection="Key pull gap exceeds motion allowance"))
    results.append(run("reject-key-front-corner-skin", "base", ("front_width=33.4",),
                       rejection="Front corner relief requires a wider nose"))
    for name, definition, rejection in (
            ("beam-shrink", "carrier_beam_width=1.1", "carrier_beam_width >= carrier_side_width"),
            ("pillar-shrink", "carrier_front_width=1.1", "carrier_front_width >= carrier_side_width"),
            ("bridge-too-low", "carrier_bridge_bottom=8.1", "carrier_bridge_bottom >="),
            ("beam-too-high", "carrier_beam_top=9.6", "carrier_beam_top <= split_height"),
            ("pillar-enters-key", "carrier_front_width=2.6", "Carrier front pillar enters key post clearance")):
        results.append(run("reject-carrier-"+name, "carrier-left", (definition,), rejection=rejection))
    for part in ("assembled", "exploded", "section"):
        results.append(run(part, part, preview=True))
    results.append(run("sensor", "sensor", preview=True))
    results.append(run("usb", "usb", preview=True))
    results.append(run("pads-installed", "pads-installed", preview=True))
    results.append(run("lid-front-detail", "lid-front-detail", preview=True))
    for part in ("front-key-base", "front-key-lid", "carrier-v10-left"):
        results.append(run(part, part, preview=True))

    shoulder = values["carrier_shoulder_z"]
    front_top = shoulder + values["front_ear_thickness"]
    rear_top = front_top + values["rear_ear_thickness"]
    results.append({"name": "screw-clamp-load-path", "passed":
                    abs(rear_top-values["split_height"]) < 1e-6 and
                    values["lid_perimeter_gap"] > 0 and
                    values["guide_top_clearance"] > 0,
                    "shoulder_z": shoulder, "carrier_top_and_lid_boss_bottom": rear_top,
                    "perimeter_gap": values["lid_perimeter_gap"],
                    "limit": "Nominal stack only; physical contact and screw preload untested"})
    compression = [1-values["pad_compressed_thickness"]/values[key]
                   for key in ("pad_min_thickness", "pad_free_thickness", "pad_max_thickness")]
    results.append({"name": "pad-thickness-preload-budget", "passed":
                    0 < compression[0] <= compression[1] <= compression[2] < 0.30,
                    "compression_fraction_min_nominal_max": compression,
                    "sensor_nominal_axial_free_gap": values["sensor_axial_play"],
                    "limit": "Thickness budget only; no clamp force or compression-set proof"})
    ring_area = values["sensor_pad_outer"]**2-values["sensor_pad_free_inner"]**2
    ring_bulge = math.sqrt((values["sensor_body_diameter"]+0.1)**2 +
                          ring_area*values["pad_max_thickness"]/values["pad_compressed_thickness"])
    rear_bulge = values["rear_pad_width"]*math.sqrt(
        values["pad_max_thickness"]/values["pad_compressed_thickness"])
    results.append({"name": "silicone-side-expansion-space", "passed":
                    ring_bulge < values["sensor_pad_installed_envelope"] <
                    values["sensor_pad_groove_outer"] and
                    rear_bulge < values["rear_pad_pocket_width"] and
                    rear_bulge < values["rear_pad_pocket_height"],
                    "ring_constant_volume_outer_max": ring_bulge,
                    "rear_pad_constant_volume_width_max": rear_bulge,
                    "limit": "Volume-space budget at nominal 0.42 mm gap, not a deformation simulation"})

    if source_hashes != {name: hashlib.sha256((ROOT/name).read_bytes()).hexdigest()
                         for name in sources}:
        raise RuntimeError("Design source changed during verification; rerun the checks")
    report = {"units": "mm", "revision": "V11-reinforced-carriers-fit-prototype", "checks": results,
              "source_sha256": source_hashes, "scad_sha256": scad_hash,
              "meshes": meshes, "all_passed": all(item["passed"] for item in results),
              "limits": ["USB uses reported 8 mm projection, 0..3 mm root height and standard width envelope",
                         "No custom-board, mated-plug or wire geometry in official STEP",
                         "Conservative component boxes plus nominal drawing PCB, not STL booleans",
                         "Native board outline differs from nominal by up to 0.014 mm",
                         "Reported 14/11/8 mm dimensions are not independently measured",
                         "Guide-hole fit is a finished-fit target requiring post-fit inspection",
                         "Silicone preload and flange/PCB tolerance stacks need physical acceptance",
                         "Discrete loading positions do not prove all continuous motion",
                         "No physical fit, strength, electrical or lifetime test performed"]}
    (BUILD / "v11-geometry-checks.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"all_passed": report["all_passed"],
                      "failed": [item for item in results if not item["passed"]]}, indent=2))
    if not report["all_passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
