# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Frozen V7 rib cores for material-containment checks, not strength estimates."""

RIB_REFERENCE = r'''
module v7_rib_core(kind) {
    // Keep the V7 X/Z profiles; the outer pair uses the V8 lateral position.
    intersection() {
        slab(0,case_height);
        translate([0.8,-20,0]) cube([6,40,17.4]);
        for (side=[-1,1])
            if (kind == "upper")
                translate([0,side*5.4+0.5,0]) rotate([90,0,0]) linear_extrude(1)
                    polygon([[0.79,13.7],[1.7,13.7],[4.8,17.41],[0.79,17.41]]);
            else if (kind == "outer")
                translate([0,side*11+0.8,0]) rotate([90,0,0]) linear_extrude(1.6)
                    polygon([[0.79,12.1],[0.79,17.41],[6.8,17.41]]);
            else assert(false,"Unknown V7 rib core");
    }
}
'''
