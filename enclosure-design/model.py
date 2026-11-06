#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Generate a dimension-unconfirmed OpenSCAD envelope for layout review."""

import argparse
import json
from pathlib import Path

from board_reference import STEP_SHA256


# Manual V1.0.2, page 9; the physical module still needs measurement.
MODULE_DIAMETER = 21.0
MODULE_HEIGHT = 6.3

# Layout allowances, not measurements of the user's assembled hardware.
PARAMETERS = {
    "case_height": 18.0,
    "case_width": 34.0,
    "front_width": 24.0,
    "front_radius": 5.0,
    "shoulder_radius": 5.0,
    "shoulder_x": 18.0,
    "wall": 1.6,
    "split_height": 9.0,
    "pcb_x": 6.0,
    "pcb_length": 24.0,
    "pcb_width": 19.0,
    "pcb_bottom": 3.5,
    "pcb_stack_height": 10.5,
    "sensor_x": 47.0,
    "sensor_diameter": MODULE_DIAMETER,
    "sensor_height": MODULE_HEIGHT,
    "usb_z": 12.0,
    "usb_width": 9.4,
    "usb_height": 3.8,
    "boss_diameter": 5.8,
    "insert_hole_diameter": 3.2,
    "insert_hole_depth": 4.6,
    "screw_clearance": 2.3,
    "head_diameter": 4.2,
    "head_recess": 1.8,
    "pcb_clearance": 0.6,
    "insulation_wall": 1.2,
    "sensor_aperture_clearance": 0.3,
    "assembly_envelope_only": 0,
    "use_board_reference": 0,
    "front_access_mockup": 0,
    "board_top_z": 0,
    "controls": [[19, -3.8], [19, 3.8]],
    "led_xy": [23, 0],
    "nose_wall": 1.6,
    "guard_x": 12,
    "screws": [[19, -13.6], [19, 13.6], [54, -12], [54, 12]],
}

COMPACT_PARAMETERS = {
    **PARAMETERS,
    "case_width": 26.0,
    "front_width": 26.0,
    "front_radius": 3.0,
    "shoulder_radius": 3.0,
    "pcb_x": 7.0,
    "sensor_x": 45.0,
    "screws": [[3.2, -8.5], [3.2, 8.5], [34.8, -10], [34.8, 10]],
}

# Board outline: official drawing and native PCB; thickness: user upper estimate.
ASSEMBLY_HEIGHT_UPPER = 14.0
ASSEMBLY_CLEARANCE = 0.9
CASE_HEIGHT = 2*PARAMETERS["wall"] + 2*ASSEMBLY_CLEARANCE + ASSEMBLY_HEIGHT_UPPER
BOARD_FACE_ABOVE_LOWEST = 11.0
USB_PROTRUSION = 8.0
USB_STANDARD_WIDTH = 8.25
USB_STANDARD_THICKNESS = 2.40
USB_BLOCKING_LIMIT = 1.0
USB_BLOCKING_RESERVE = 0.2
NOSE_WALL = USB_BLOCKING_LIMIT - USB_BLOCKING_RESERVE
# The front bare board edge locates against the insulating nose in the trial gauge.
FRONT_BOARD_CLEARANCE = 0.0
DOCUMENTED_PARAMETERS = {
    **COMPACT_PARAMETERS,
    "case_height": CASE_HEIGHT,
    "split_height": CASE_HEIGHT/2,
    "pcb_length": 23.5,
    "pcb_width": 18.0,
    "pcb_bottom": PARAMETERS["wall"] + ASSEMBLY_CLEARANCE,
    "pcb_stack_height": ASSEMBLY_HEIGHT_UPPER,
    "assembly_envelope_only": 1,
}


def reference_parameters():
    folder = Path(__file__).resolve().parent / "build" / "reference"
    data = json.loads((folder / "board-geometry.json").read_text())
    if data["step_sha256"] != STEP_SHA256 or not (folder / "board.stl").is_file():
        raise ValueError("Generate the verified board reference before V4")
    if data["units"] != "mm" or not data["brep_valid"] or data["mesh_flags"] != 0:
        raise ValueError("Board reference failed geometry checks")
    nodes = {node["path"]: node for node in data["nodes"]}
    pcb_x = NOSE_WALL + FRONT_BOARD_CLEARANCE
    if pcb_x + USB_BLOCKING_RESERVE > USB_BLOCKING_LIMIT:
        raise ValueError("USB axial blocking exceeds the 1 mm limit including reserve")
    shift = DOCUMENTED_PARAMETERS["pcb_x"] - pcb_x
    values = {
        **DOCUMENTED_PARAMETERS,
        "pcb_x": pcb_x,
        "sensor_x": DOCUMENTED_PARAMETERS["sensor_x"] - shift,
        "screws": [[s[0]-shift, s[1]] for s in DOCUMENTED_PARAMETERS["screws"][2:]],
        "front_radius": 2.0,
        "nose_wall": NOSE_WALL,
        "guard_x": pcb_x + 5,
        "usb_blocking_limit": USB_BLOCKING_LIMIT,
        "usb_blocking_reserve": USB_BLOCKING_RESERVE,
    }

    def case_xy(bounds):
        x = (bounds[0] + bounds[3])/2
        y = (bounds[1] + bounds[4])/2
        return [values["pcb_x"] + values["pcb_length"] - y,
                x - values["pcb_width"]/2]

    top_z = values["pcb_bottom"] + BOARD_FACE_ABOVE_LOWEST
    if top_z + data["bounds"][5] > values["pcb_bottom"] + ASSEMBLY_HEIGHT_UPPER:
        raise ValueError("Known board geometry exceeds the reported stack height")
    return {
        **values,
        "use_board_reference": 1,
        "front_access_mockup": 1,
        "board_top_z": top_z,
        "usb_width": 9.0,
        "controls": [case_xy(nodes[f"/PCB/Key{i}"]["actuator_top_bounds"])
                     for i in (1, 2)],
        "led_xy": case_xy(nodes["/PCB/L2"]["bounds"]),
    }


MODEL = r'''
part = "exploded";
collision_offset = [0, 0, 0];
$fn = 64;
eps = 0.02;
seat_z = case_height - sensor_height - 1.2;
top_floor = case_height - wall;

assert(wall > 0 && split_height > wall);
assert(case_height > split_height + insert_hole_depth + wall);
assert(pcb_bottom > wall && pcb_bottom + pcb_stack_height < top_floor);
assert(sensor_x - sensor_diameter/2 > pcb_x + pcb_length);
assert((boss_diameter - insert_hole_diameter)/2 >= 1.2);
assert(split_height + insert_hole_depth < top_floor);
if (use_board_reference)
    assert(pcb_x + usb_blocking_reserve <= usb_blocking_limit,
           "USB axial blocking exceeds the 1 mm limit including reserve");
for (s = screws) {
    dx = max(pcb_x - s[0], 0, s[0] - pcb_x - pcb_length);
    dy = max(abs(s[1]) - pcb_width/2, 0);
    assert(sqrt(dx*dx + dy*dy) > boss_diameter/2 + pcb_clearance);
    assert(norm(s - [sensor_x, 0]) > sensor_diameter/2 +
           sensor_aperture_clearance + insert_hole_diameter/2 + insulation_wall);
    if (s[0] <= pcb_x + 1)
        assert(abs(s[1]) > usb_width/2 + insert_hole_diameter/2 + insulation_wall);
}
echo("DRAFT: layout allowances only; retention, fits and isolation unverified.");

module outline(inset = 0) {
    offset(delta = -inset)
        hull() {
            for (y = [-1, 1])
                translate([front_radius, y * (front_width/2 - front_radius)])
                    circle(r = front_radius);
            for (y = [-1, 1])
                translate([shoulder_x, y * (case_width/2 - shoulder_radius)])
                    circle(r = shoulder_radius);
            translate([sensor_x, 0]) circle(r = case_width/2);
        }
}

module slab(z, height, inset = 0) {
    translate([0, 0, z]) linear_extrude(height) outline(inset);
}

module usb_opening() {
    if (front_access_mockup)
        translate([-eps, -usb_width/2, pcb_bottom - 0.3])
            cube([pcb_x + 1, usb_width, pcb_stack_height + 0.6]);
    else
        translate([-eps, -usb_width/2, usb_z - usb_height/2])
            cube([pcb_x + 1, usb_width, usb_height]);
}

module front_pcb_relief() {
    if (front_access_mockup)
        translate([nose_wall, -pcb_width/2 - pcb_clearance, pcb_bottom - 0.3])
            cube([pcb_x + 2, pcb_width + 2*pcb_clearance, pcb_stack_height + 0.6]);
}

module lower_bosses() {
    for (s = screws)
        translate([s[0], s[1], wall - eps])
            cylinder(d = boss_diameter, h = split_height - wall + eps);
}

module upper_bosses() {
    for (s = screws)
        translate([s[0], s[1], split_height])
            cylinder(d = boss_diameter, h = top_floor - split_height + eps);
}

module pcb_guards() {
    for (y = [-1, 1])
        translate([guard_x, y > 0 ? 10.5 : -11.7, wall - eps])
            cube([14, 1.2, split_height - wall + eps]);
}

module sensor_seat() {
    difference() {
        translate([sensor_x, 0, seat_z])
            cylinder(d = sensor_diameter + 3, h = 1.2);
        translate([sensor_x, 0, seat_z - eps])
            cylinder(d = 17.6, h = 1.2 + 2*eps);
    }
    for (p = [[sensor_x, -10.6], [sensor_x, 10.6], [sensor_x + 10.6, 0]])
        translate([p[0] - 1, p[1] - 1, wall - eps])
            cube([2, 2, seat_z - wall + 1.2 + eps]);
}

module base() {
    difference() {
        union() {
            difference() {
                slab(0, split_height);
                slab(wall, split_height, wall);
            }
            lower_bosses();
            pcb_guards();
            sensor_seat();
        }
        for (s = screws) {
            translate([s[0], s[1], -eps])
                cylinder(d = screw_clearance, h = split_height + 2*eps);
            translate([s[0], s[1], -eps])
                cylinder(d = head_diameter, h = head_recess + eps);
        }
        usb_opening();
        front_pcb_relief();
    }
}

module lid() {
    difference() {
        union() {
            difference() {
                slab(split_height, case_height - split_height);
                slab(split_height - eps, top_floor - split_height + eps, wall);
            }
            upper_bosses();
        }
        for (s = screws)
            translate([s[0], s[1], split_height - eps])
                cylinder(d = insert_hole_diameter, h = insert_hole_depth + eps);
        translate([sensor_x, 0, split_height - eps])
            cylinder(d = sensor_diameter + 2*sensor_aperture_clearance,
                     h = case_height - split_height + 2*eps);
        for (p = controls)
            translate([p[0], p[1], top_floor - eps])
                cylinder(d = 3.2, h = wall + 2*eps);
        translate([led_xy[0], led_xy[1], top_floor - eps])
            cylinder(d = 2, h = wall + 2*eps);
        usb_opening();
        front_pcb_relief();
    }
}

module components() {
    if (use_board_reference) {
        %translate([pcb_x, -pcb_width/2, pcb_bottom])
            cube([pcb_length, pcb_width, pcb_stack_height]);
        color([0.12, 0.30, 0.46])
            multmatrix([[0, -1, 0, pcb_x + pcb_length],
                        [1, 0, 0, -pcb_width/2],
                        [0, 0, 1, board_top_z], [0, 0, 0, 1]])
                import("reference/board.stl");
    } else if (assembly_envelope_only) {
        color([0.12, 0.30, 0.46, 0.35])
            translate([pcb_x, -pcb_width/2, pcb_bottom])
                cube([pcb_length, pcb_width, pcb_stack_height]);
    } else {
        color([0.12, 0.30, 0.46]) {
            translate([pcb_x, -pcb_width/2, pcb_bottom]) cube([pcb_length, pcb_width, 1.6]);
            translate([pcb_x, -pcb_width/2, pcb_bottom + pcb_stack_height - 1.6])
                cube([pcb_length, pcb_width, 1.6]);
        }
        color([0.16, 0.17, 0.18])
            for (y = [-1, 1])
                translate([pcb_x + 2, y > 0 ? 7 : -9, pcb_bottom + 1.6])
                    cube([pcb_length - 4, 2, pcb_stack_height - 3.2]);
    }
    if (!use_board_reference)
        color([0.68, 0.69, 0.71])
            translate([-8, -4.2, usb_z - 1.4]) cube([pcb_x + 8, 8.4, 2.8]);
    color([0.78, 0.79, 0.79])
        translate([sensor_x, 0, case_height - sensor_height])
            cylinder(d = sensor_diameter, h = sensor_height);
    color([0.1, 0.11, 0.12])
        translate([sensor_x, 0, case_height]) cylinder(d = 17, h = 0.25);
    color([0.25, 0.55, 0.52])
        hull() {
            translate([pcb_x + pcb_length - 1, 0, pcb_bottom + 3]) sphere(r = 1.6);
            translate([sensor_x - 3, 0, seat_z - 2]) sphere(r = 1.6);
        }
}

module assembly(explode = 0) {
    color([0.26, 0.28, 0.31]) base();
    translate([0, 0, explode/2]) components();
    color([0.45, 0.47, 0.5]) translate([0, 0, explode]) lid();
}

if (part == "base") base();
else if (part == "lid") lid();
else if (part == "assembled") assembly();
else if (part == "exploded") assembly(21);
else if (part == "section") {
    difference() {
        union() {
            color([0.26, 0.28, 0.31]) base();
            color([0.45, 0.47, 0.5]) lid();
        }
        translate([-10, -25, -1]) cube([85, 25, 55]);
    }
    components();
}
else if (part == "envelope-collision") {
    intersection() {
        union() { base(); lid(); }
        translate([pcb_x, -pcb_width/2, pcb_bottom] + collision_offset)
            cube([pcb_length, pcb_width, pcb_stack_height]);
    }
}
else if (part == "front-gauge") {
    assert(front_access_mockup, "Front gauge belongs to V4");
    difference() {
        intersection() {
            union() { base(); lid(); }
            translate([-eps, -case_width, -eps])
                cube([6 + eps, 2*case_width, case_height + 2*eps]);
        }
        for (y = [-1, 1])
            translate([4, y > 0 ? case_width/2 - 0.3 : -case_width/2 - eps,
                       board_top_z - 0.15])
                cube([2 + eps, 0.3 + eps, 0.3]);
    }
}
else assert(false, "Unknown part");
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--variant", choices=("v1", "v2", "v3", "v4"), default="v4")
    args = parser.parse_args()
    values = (reference_parameters() if args.variant == "v4" else
              {"v1": PARAMETERS, "v2": COMPACT_PARAMETERS,
               "v3": DOCUMENTED_PARAMETERS}[args.variant])
    folder = Path(__file__).resolve().parent / "build"
    folder.mkdir(exist_ok=True)
    header = (
        "// SPDX-License-Identifier: AGPL-3.0-only\n"
        "// Copyright (C) 2026 RS-Key contributors\n\n"
        f"// DRAFT: not dimensioned for manufacture. See ../concept-{args.variant}.md.\n"
    )
    parameters = "\n".join(f"{name} = {value};" for name, value in values.items())
    destination = folder / f"concept-{args.variant}.scad"
    destination.write_text(header + parameters + "\n" + MODEL, encoding="utf-8")
    print(destination)


if __name__ == "__main__":
    main()
