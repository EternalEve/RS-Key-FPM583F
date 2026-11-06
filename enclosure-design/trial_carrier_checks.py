# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Exercise added carrier sections and preservation through the geometry gate."""


def carrier_checks():
    jobs = []
    probes = (("toe-depth", (1.2, 10.6, 10.3)),
              ("front-pillar", (2.0, 11.9, 9.0)),
              ("side-beam", (12, 11.2, 5)),
              ("raised-bridge", (5, 10.4, 9)),
              ("toe-root", (1.6, 10.1, 9.8)),
              ("toe-step-root", (1.1, 9.5, 10.8)))
    for side, label, piece in ((-1, "left", 2), (1, "right", 3)):
        jobs.append((label+"-carrier-v10-core-preserved", "carrier-core-loss",
                     (f"key_side={side}",), True))
        for name, (x, y, z) in probes:
            definitions = (f"probe_piece={piece}", f"probe_point=[{x},{side*y},{z}]")
            jobs.extend(((f"{label}-carrier-{name}", "root-probe", definitions, False),
                         (f"{label}-carrier-{name}-missing-control", "root-probe",
                          definitions+("carrier_reinforcement=false",), True)))
    return jobs
