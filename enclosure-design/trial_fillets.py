# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Local additive root blends for the enclosure's load-bearing joints."""

FILLETS = r'''
module root_profile(r) {
    difference() {
        translate([-eps,-eps]) square([r+eps,r+eps]);
        translate([r,r]) circle(r=r);
    }
}

module root_strip(origin, u, v, axis, length, r, extra=0) {
    if (r > 0)
        multmatrix([[u[0],v[0],axis[0],origin[0]],
                    [u[1],v[1],axis[1],origin[1]],
                    [u[2],v[2],axis[2],origin[2]], [0,0,0,1]])
            linear_extrude(length) offset(delta=extra) root_profile(r);
}

module boss_root(z, down=false) {
    intersection() {
        slab(0,case_height);
        for (s=screws) translate([s[0],s[1],z])
            scale([1,1,down ? -1 : 1]) rotate_extrude()
                translate([boss_diameter/2,0]) root_profile(boss_root_radius);
    }
}

module perimeter_root(z, down=false) {
    if (shell_root_radius > 0)
        translate([0,0,z]) scale([1,1,down ? -1 : 1]) difference() {
            slab(0,shell_root_radius);
            for (i=[0:fillet_arc_steps-1]) hull() {
                for (j=[i,i+1]) {
                    a=90*j/fillet_arc_steps;
                    h=shell_root_radius*(1-cos(a));
                    inset=wall+shell_root_radius*(1-sin(a));
                    translate([0,0,h-eps]) linear_extrude(2*eps) outline(inset);
                }
            }
        }
}

module gusset_profile(points, r, floor_z) {
    // Keep the core explicitly; the closing contributes only additional material.
    union() {
        polygon(points);
        offset(r=-r) offset(r=r) union() {
            polygon(points);
            translate([-pcb_x,floor_z]) square([pcb_x,case_height-floor_z]);
            translate([-pcb_x,top_floor])
                square([front_rib_length+pcb_x+2*r,wall]);
        }
    }
}

module root_gusset(y, width, points, r, floor_z) {
    translate([pcb_x,y+width/2,0]) rotate([90,0,0])
        linear_extrude(width) gusset_profile(points,r,floor_z);
    intersection() {
        translate([pcb_x,y+width/2+r,0]) rotate([90,0,0])
            linear_extrude(width+2*r) gusset_profile(points,r,floor_z);
        union() {
            for (side=[-1,1]) {
                edge=y+side*width/2;
                root_strip([pcb_x,edge,floor_z], [1,0,0], [0,side,0], [0,0,1],
                           top_floor-floor_z+eps,r);
                root_strip([pcb_x,edge,top_floor], [0,side,0], [0,0,-1], [1,0,0],
                           front_rib_length+r,r);
            }
        }
    }
}

module front_roof_root() {
    root_strip([pcb_x,-frame_half_width,top_floor], [1,0,0], [0,0,-1], [0,1,0],
               2*frame_half_width,front_roof_radius);
}

module carrier_ear_root(extra=0) {
    rail_end=screws[0][0]-boss_diameter/2-fit_clearance;
    for (side=[-1,1])
        root_strip([rail_end,side > 0 ? carrier_side_inner-extra :
                   -carrier_side_inner-carrier_side_width-extra,carrier_shoulder_z],
                   [1,0,0], [0,0,-1], [0,1,0], carrier_side_width+2*extra,
                   carrier_root_radius,extra);
}

module carrier_front_roots() {
    for (side=[-1,1]) {
        root_strip([pcb_x+front_stand_depth,side*front_stand_inner,slot_bottom-1],
                   [0,-side,0], [0,0,-1], [1,0,0],
                   front_tab_depth-front_stand_depth,carrier_root_radius);
        root_strip([pcb_x+front_stand_depth,side > 0 ? front_stand_inner :
                    -front_stand_inner-carrier_side_width,slot_bottom-1],
                   [-1,0,0], [0,0,-1], [0,1,0], carrier_side_width,front_tab_root_radius);
    }
}
'''
