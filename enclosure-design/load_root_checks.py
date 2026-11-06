# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Probe additive roots and preserve this revision's unblended load-bearing cores."""

import json


def root_probes(p):
    wall = p["wall"]
    roof = p["case_height"]-wall
    ring_bottom = p["case_height"]-p["sensor_flange_depth"]-p["sensor_flange_thickness"]-1.2
    rear_x = p["pcb_x"]+p["pcb_length"]+p["pcb_axial_stop_gap"]
    probes = []
    for side in (-1, 1):
        probes.extend((
            (f"front-corner-{side}", 0, [p["pcb_x"]+0.05,side*(p["front_pocket_half"]-0.05),5]),
            (f"lid-bearing-{side}", 1, [p["pcb_x"]+p["front_bearing_rib_length"]+0.03,
                                       side*p["front_tab_y"],roof-0.03]),
            (f"lid-stiffener-{side}", 1, [10,side*9.85,roof-0.05]),
            (f"rear-foot-{side}", 2 if side < 0 else 3, [rear_x+p["rear_post_depth"]+0.05,side*p["rear_stand_y"],wall+0.95]),
            (f"rear-ear-{side}", 2 if side < 0 else 3,
             [rear_x+p["rear_post_depth"]+0.05,side*p["rear_stand_y"],p["carrier_shoulder_z"]+p["front_ear_thickness"]-0.05]),
        ))
    for index in range(3):
        center = ([p["sensor_x"]+p["sensor_leg_tangent"]/2+0.15,
                   (-1 if index == 0 else 1)*p["sensor_leg_radius"]] if index < 2 else
                  [p["sensor_x"]+p["sensor_leg_radius"],p["sensor_leg_tangent"]/2+0.15])
        probes.extend(((f"sensor-foot-{index}", 0, [*center,wall+0.05]),
                       (f"sensor-ring-{index}", 0, [*center,ring_bottom-0.05])))
        stem = [center[0]-0.1,center[1],8] if index < 2 else [center[0],center[1]-0.1,8]
        probes.append((f"sensor-stem-{index}",0,stem))
    return probes


def jobs(p):
    checks = []
    for index, part in enumerate(("base", "lid", "carrier-left", "carrier-right")):
        definition = (f"root_piece={index}",)
        checks.extend(((f"load-roots-retain-{part}-core", "load-root-material-loss", definition, True),
                       (f"load-roots-add-to-{part}", "load-root-added-material", definition)))
    for name, part, center in root_probes(p):
        checks.append((f"root-{name}-has-material", "load-root-probe",
                       (f"root_piece={part}",f"root_probe_center={json.dumps(center)}")))
    checks.extend((
        ("front-corner-0.2mm-loading-reserve", "front-corner-clearance", (), True),
        ("front-corner-R2.0-invades-loading-reserve", "front-corner-clearance", ("front_corner_radius=2.0",)),
        ("front-corner-round-interior-is-air", "load-root-probe",
         (f'root_probe_center={[p["pcb_x"]+0.4,p["front_pocket_half"]-0.4,5]}',), True),
        ("sensor-foot-round-interior-is-air", "load-root-probe",
         (f'root_probe_center={[p["sensor_x"]+p["sensor_leg_tangent"]/2+0.48,-p["sensor_leg_radius"],p["wall"]+0.48]}',), True),
    ))
    return checks
