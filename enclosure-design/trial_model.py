#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Generate the V11 prototype with restored rib cores and explicit design inputs."""

import json
from pathlib import Path

from model import reference_parameters
from trial_geometry import MODEL


ROOT = Path(__file__).resolve().parent


def parameters():
    original = reference_parameters()
    shift = 0.0
    reference = json.loads((ROOT / "build/reference/board-geometry.json").read_text())
    components = [node["bounds"] for node in reference["nodes"]
                  if node["path"].count("/") == 2 and node["path"] != "/PCB/Board"]
    if len(components) < 30:
        raise ValueError("Official component bounding boxes are incomplete")
    return {
        **original,
        "pcb_x": original["pcb_x"] + shift,
        "nose_wall": 0.8,
        "front_width": 33.6,
        "sensor_x": original["sensor_x"] + shift,
        "controls": [[p[0]+shift, p[1]] for p in original["controls"]],
        "led_xy": [original["led_xy"][0]+shift, original["led_xy"][1]],
        "screws": [[28.5, -9.8], [28.5, 9.8]],
        "usb_blocking_reserve": 0.20,
        "fixture_motion_allowance": 0.10,
        "manufacturing_reserve": 0.10,
        "boss_diameter": 6.4,
        "insert_hole_depth": 5.0,
        "insert_outer_diameter": 3.6,
        "insert_length": 4.0,
        "head_diameter": 4.4,
        "head_recess": 2.2,
        "screw_head_height": 2.0,
        "screw_head_outer": 3.8,
        "screw_length": 10.0,
        "pcb_thickness": 1.6,
        "pcb_slot_clearance": 0.20,
        "pcb_axial_stop_gap": 0.05,
        "front_tab_y": 5.4,
        "front_tab_width": 1.0,
        "front_tab_depth": 1.05,
        "rear_tab_depth": 0.4,
        "rear_stand_y": 5.7,
        "rear_stand_width": 2.4,
        "frame_half_width": 14.6,
        "front_keys_enabled": True,
        "front_key_shoulders": True,
        "front_key_y": 13.5,
        "front_key_width": 1.4,
        "front_key_head_width": 2.6,
        "front_key_head_x": 4.8,
        "front_key_head_depth": 2.0,
        "front_key_post_depth": 1.2,
        "front_key_z": 1.8,
        "front_key_height": 6.0,
        "front_key_socket_top": 8.0,
        "front_key_pull_gap": 0.1,
        "front_key_shoulder": 1.6,
        "front_key_inner_wall": 1.0,
        "front_key_outer_wall": 1.2,
        "front_key_corner_radius": 0.15,
        "front_key_blend_radius": 0.25,
        "front_key_root_radius": 0.4,
        "front_key_transition_radius": 0.6,
        "front_key_lead": 0.2,
        "front_rib_width": 1.6,
        "front_rib_length": 6.0,
        "front_rib_bottom": 12.1,
        "front_bearing_rib_length": 4.0,
        "front_rib_inner": 10.2,
        "front_stand_inner": 10.2,
        "front_stand_depth": 0.6,
        "front_upper_toe_start": 0.9,
        "front_upper_toe_depth": 1.2,
        "front_upper_root_rise": 0.0,
        "front_rib_radius": 0.8,
        "front_upper_radius": 0.4,
        "front_roof_radius": 0.6,
        "shell_root_radius": 0.6,
        "boss_root_radius": 0.8,
        "carrier_root_radius": 0.6,
        "front_tab_root_radius": 0.4,
        "fillet_arc_steps": 16,
        "carrier_side_inner": 9.6,
        "carrier_side_width": 1.2,
        "carrier_reinforcement": True,
        "carrier_beam_width": 2.0,
        "carrier_front_width": 2.0,
        "carrier_toe_root_thickness": 2.0,
        "carrier_beam_top": 9.5,
        "carrier_bridge_bottom": 8.4,
        "carrier_step_radius": 0.4,
        "carrier_split_gap": 0.2,
        "usb_width": 9.2,
        "usb_reference_width": 8.25,
        "usb_protrusion": 8.0,
        "usb_root_bottom_offset": 0.0,
        "usb_root_top_offset": 3.0,
        "usb_vertical_clearance": 0.25,
        "fit_clearance": 0.2,
        "tongue_height": 1.0,
        "tongue_width": 0.7,
        "sensor_body_diameter": 19.0,
        "sensor_flange_depth": 2.1,
        "sensor_flange_thickness": 1.0,
        "sensor_front_hole": 19.4,
        "sensor_flange_hole": 21.5,
        "sensor_cradle_outer": 22.2,
        "sensor_axial_play": 0.0,
        "sensor_leg_radius": 10.4,
        "sensor_leg_width": 1.4,
        "sensor_leg_tangent": 2.4,
        "lid_perimeter_gap": 0.1,
        "carrier_shoulder_z": 7.0,
        "front_ear_thickness": 1.2,
        "rear_ear_thickness": 1.3,
        "guide_diameter": 4.8,
        "guide_hole_diameter": 4.9,
        "guide_top_clearance": 0.2,
        "pad_free_thickness": 0.508,
        "pad_min_thickness": 0.457,
        "pad_max_thickness": 0.584,
        "pad_compressed_thickness": 0.42,
        "rear_pad_width": 1.2,
        "rear_pad_height": 1.2,
        "rear_pad_pocket_width": 1.6,
        "rear_pad_pocket_height": 1.6,
        "top_pad_length": 0.8,
        "top_pad_width": 12.0,
        "sensor_pad_outer": 20.5,
        "sensor_pad_free_inner": 18.8,
        "sensor_pad_groove_outer": 21.6,
        "sensor_pad_installed_envelope": 21.5,
        "sensor_lip_outer": 22.8,
        "official_components": components,
    }


def main():
    values = parameters()
    folder = ROOT / "build"
    folder.mkdir(exist_ok=True)
    header = (
        "// SPDX-License-Identifier: AGPL-3.0-only\n"
        "// Copyright (C) 2026 RS-Key contributors\n"
        "// V11 FIT PROTOTYPE. See concept-v11.md; extraction strength needs physical testing.\n"
    )
    assignments = "\n".join(f"{key} = {json.dumps(value)};"
                            for key, value in values.items())
    destination = folder / "concept-v11.scad"
    destination.write_text(header + assignments + "\n" + MODEL, encoding="utf-8")
    (folder / "v11-parameters.json").write_text(
        json.dumps(values, indent=2) + "\n", encoding="utf-8")
    print(destination)


if __name__ == "__main__":
    main()
