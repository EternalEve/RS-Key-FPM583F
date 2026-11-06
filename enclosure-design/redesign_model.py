#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Generate the V16 load-root candidate without replacing archived print kits."""

import json

from redesign_geometry import MODEL
from trial_model import ROOT, parameters as trial_parameters


def parameters():
    previous = trial_parameters()
    minimum_wall = 1.2
    return {
        **previous,
        "front_width": 28.0,
        "front_radius": 1.4,
        "shoulder_x": 28.5,
        "body_width": 28.0,
        "front_keys_enabled": False,
        "front_key_shoulders": False,
        "front_pocket_half": 12.4,
        "front_pocket_depth": 3.6,
        "lid_seam_gap": 0.1,
        "lid_guide_gap": 0.3,
        "lid_guide_width": 1.2,
        "lid_guide_depth": 1.0,
        "front_stop_enabled": True,
        # Stay below the 0.475 mm side margin to retain the USB's bottom clearance.
        "usb_slot_bottom_radius": 0.4,
        "load_roots_enabled": True,
        "front_corner_radius": 0.5,
        "sensor_foot_radius": 0.6,
        "sensor_ring_root_radius": 0.5,
        "sensor_stem_blend": 0.1,
        "lid_rib_root_radius": 0.4,
        "rear_carrier_root_radius": 0.4,
        "screws": [[28.5, -10.85], [28.5, 10.85]],
        "manufacturing_wall_min": minimum_wall,
        # Keep a margin around the faceted circular wall, beyond the checked section.
        "carrier_ear_diameter": previous["guide_hole_diameter"]+2*(minimum_wall+0.05),
        "mount_shell_wall": 1.3,
        "mount_bulges": True,
        "mount_blend_radius": 0.6,
        "local_wall_roots": True,
        "rear_stand_y": 6.5,
        "rear_stand_width": previous["rear_pad_pocket_width"]+2*minimum_wall,
        "rear_post_depth": 1.65,
        "rear_upper_y": 5.7,
        "rear_upper_width": 2.4,
        "rear_top_thickness": 1.6,
        "rear_foot_half_width": 9.0,
        "top_pad_end_clearance": 0.1,
    }


def main():
    build = ROOT / "build"
    build.mkdir(exist_ok=True)
    values = parameters()
    header = (
        "// SPDX-License-Identifier: AGPL-3.0-only\n"
        "// Copyright (C) 2026 RS-Key contributors\n"
        "// V16 LOAD-ROOT CANDIDATE: physical strength and fit need prototype testing.\n"
    )
    assignments = "\n".join(f"{key} = {json.dumps(value)};" for key, value in values.items())
    (build / "concept-v16.scad").write_text(header + assignments + "\n" + MODEL)
    (build / "v16-parameters.json").write_text(json.dumps(values, indent=2) + "\n")
    print(build / "concept-v16.scad")


if __name__ == "__main__":
    main()
