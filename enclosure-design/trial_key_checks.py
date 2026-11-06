# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Contact and missing-feature controls for the enclosure's lower front keys."""


def key_checks():
    jobs = []
    for side, label in ((-1, "left"), (1, "right")):
        definitions = (f"key_side={side}",)
        pull = definitions + ("key_shift=[-0.15,0,0]",)
        jobs.extend((
            (f"{label}-key-seated-clear", "front-key-collision", definitions, True),
            (f"{label}-key-entry-3mm-clear", "front-key-collision",
             definitions + ("key_shift=[0,0,3]",), True),
            (f"{label}-key-allows-0.08mm-pull", "front-key-collision",
             definitions + ("key_shift=[-0.08,0,0]",), True),
            (f"{label}-key-stops-0.15mm-pull", "front-key-collision", pull, False),
            (f"{label}-key-contact-only-at-head", "front-key-wrong-contact", pull, True),
            (f"{label}-key-no-shoulder-control", "front-key-collision",
             pull + ("front_key_shoulders=false",), True),
            (f"{label}-key-missing-control", "front-key-collision",
             pull + ("front_keys_enabled=false",), True),
        ))
    jobs.extend((("key-socket-core-skin", "front-key-skin-loss", (), True),
                 ("key-socket-mouth-skin", "front-key-skin-loss", ("key_mouth=true",), True)))
    roots = (("key-front-root", "[0.88,12.72,5]", "[1.1,12.5,5]", "front_key_root_radius"),
             ("key-roof-root", "[2.08,13.5,17.32]", "[2.3,13.5,17.1]", "front_key_root_radius"),
             ("key-foot-root", "[2.1,13.5,7.9]", "[2.45,13.5,8.25]", "front_key_transition_radius"))
    for name, material, air, radius in roots:
        definitions = ("probe_piece=1", f"probe_point={material}")
        jobs.extend(((name+"-material", "root-probe", definitions, False),
                     (name+"-air", "root-probe", ("probe_piece=1", f"probe_point={air}"), True),
                     (name+"-missing-control", "root-probe", definitions+(f"{radius}=0",), True)))
    return jobs
