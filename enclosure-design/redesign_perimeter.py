# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Keep convex-wall fillets and build local non-convex fillets without a hull."""

PERIMETER_ROOTS = r'''
module original_wall_root() {
    translate([0,0,wall]) difference() {
        linear_extrude(shell_root_radius) offset(delta=0) original_outline();
        for (i=[0:fillet_arc_steps-1]) hull() for (j=[i,i+1]) {
            a=90*j/fillet_arc_steps;
            translate([0,0,shell_root_radius*(1-cos(a))-eps]) linear_extrude(2*eps)
                offset(delta=-(wall+shell_root_radius*(1-sin(a)))) original_outline();
        }
    }
}
module mount_root_section(extra=0) {
    radius=carrier_ear_diameter/2+fit_clearance+mount_shell_wall+wall;
    translate([screws[0][0]-radius-extra,-case_width]) square([2*(radius+extra),2*case_width]);
}
module mount_root_window(extra=0) {
    translate([0,0,-eps]) linear_extrude(case_height) mount_root_section(extra);
}
module stepped_wall_root() {
    // A global hull would convexify the local bulges and erase distant fillets.
    translate([0,0,wall]) intersection() {
        linear_extrude(shell_root_radius) intersection() { outline(); mount_root_section(eps/10); }
        union() for (i=[0:fillet_arc_steps-1]) for (k=[0:1]) {
            a0=90*i/fillet_arc_steps;
            a1=90*(i+1)/fillet_arc_steps;
            z0=shell_root_radius*(1-cos(a0));
            z1=shell_root_radius*(1-cos(a1));
            inset0=wall+shell_root_radius*(1-sin(a0));
            inset1=wall+shell_root_radius*(1-sin(a1));
            translate([0,0,z0+(z1-z0)*k/2]) linear_extrude((z1-z0)/2+eps/100)
                intersection() {
                    mount_root_section(eps/10);
                    difference() { outline(); outline(inset0+(inset1-inset0)*k/2); }
                }
        }
    }
}
module base_wall_roots() {
    if (!local_wall_roots) perimeter_root(wall);
    else union() {
        difference() { original_wall_root(); mount_root_window(); }
        stepped_wall_root();
    }
}
'''
