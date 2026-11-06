# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Additive blends at the V15 load junctions, outside retained assembly paths."""

LOAD_ROOTS = r'''
module post_roots(center, size, z, radius, down=false, rounding=0) {
    steps=fillet_arc_steps/2;
    // One overlapping collar avoids coincident seams along the post's corners.
    translate([center[0],center[1],z]) scale([1,1,down ? -1 : 1])
        for (i=[0:steps-1]) hull() for (j=[i,i+1]) {
            a=90*j/steps;
            r=rounding+(j==steps ? 0 : radius*(1-sin(a)));
            translate([0,0,radius*(1-cos(a))-eps]) linear_extrude(2*eps)
                offset(r=r,$fn=32) square(size,center=true);
        }
}

module front_corner_roots() {
    for (s=[-1,1])
        root_strip([pcb_x,s*front_pocket_half,wall],[1,0,0],[0,-s,0],[0,0,1],
                   base_top-wall,front_corner_radius);
}

module sensor_load_roots() {
    difference() {
    for (i=[0:2]) {
        p=i==0?[sensor_x,-sensor_leg_radius]:i==1?[sensor_x,sensor_leg_radius]:[sensor_x+sensor_leg_radius,0];
        size=i==2?[sensor_leg_width,sensor_leg_tangent]:[sensor_leg_tangent,sensor_leg_width];
        translate([p[0],p[1],wall-eps]) linear_extrude(cradle_top-wall+eps)
            offset(r=sensor_stem_blend,$fn=32) square(size,center=true);
        post_roots(p,size,wall,sensor_foot_radius,rounding=sensor_stem_blend);
        intersection() {
            post_roots(p,size,cradle_top-1.2,sensor_ring_root_radius,true,sensor_stem_blend);
            translate([sensor_x,0,cradle_top-1.2-sensor_ring_root_radius-eps])
                linear_extrude(sensor_ring_root_radius+2*eps) difference() {
                    circle(d=sensor_cradle_outer);
                    circle(d=sensor_front_hole);
                }
        }
    }
    translate([sensor_x,0,0]) cylinder(d=sensor_front_hole,h=case_height);
    }
}

module lid_bearing_profile() {
    polygon([[0,slot_top],[front_upper_toe_depth,slot_top],
             [front_bearing_rib_length,top_floor+eps],[0,top_floor+eps]]);
}
module lid_load_roots() {
    for (s=[-1,1]) {
        translate([pcb_x,s*front_tab_y+front_tab_width/2,0]) rotate([90,0,0])
            linear_extrude(front_tab_width) offset(delta=eps) difference() {
                offset(r=-lid_rib_root_radius) offset(r=lid_rib_root_radius) union() {
                    lid_bearing_profile();
                    translate([0,top_floor]) square([front_bearing_rib_length+2*lid_rib_root_radius,wall]);
                }
                union() {
                    lid_bearing_profile();
                    translate([0,top_floor]) square([front_bearing_rib_length+2*lid_rib_root_radius,wall]);
                }
            }
        post_roots([(5+screws[0][0])/2,s*10.4],
                   [screws[0][0]-5,1],top_floor,lid_rib_root_radius,true);
    }
}

module rear_carrier_roots() {
    for (s=[-1,1]) {
        intersection() {
            post_roots([rear_x+rear_post_depth/2,s*rear_stand_y],[rear_post_depth,rear_stand_width],
                       wall+0.9,rear_carrier_root_radius);
            translate([0,0,wall+0.9-eps]) linear_extrude(rear_carrier_root_radius+2*eps)
                projection() rear_foot();
        }
        intersection() {
            post_roots([rear_x+rear_post_depth/2,s*rear_stand_y],[rear_post_depth,rear_stand_width],
                       front_ear_top,rear_carrier_root_radius,true);
            translate([0,0,front_ear_top-rear_carrier_root_radius-eps])
                linear_extrude(rear_carrier_root_radius+2*eps) projection() hull() {
                    translate([rear_x+1.4,s*rear_stand_y,0]) cylinder(d=1.6,h=1);
                    translate([screws[0][0],s*abs(screws[0][1]),0]) cylinder(d=carrier_ear_diameter,h=1);
                }
        }
    }
}
'''
