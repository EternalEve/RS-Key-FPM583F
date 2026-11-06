# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""OpenSCAD geometry for the V11 root-filleted prototype."""

from trial_fillets import FILLETS
from trial_rib_reference import RIB_REFERENCE
from trial_front_keys import FRONT_KEYS
from trial_carriers import CARRIERS

MODEL = r'''
// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors
// Parameters are prepended by trial_model.py; all coordinates and exports use mm.

part = "exploded";
collision_shift = [0, 0, 0];
pair = [0, 1];
pair_offset = [0, 0, 0];
assembly_lift = 0;
assembly_side_offset = 0;
probe_piece = 1;
probe_point = [0,0,0];
web_kind = "upper";
key_side = 1;
key_shift = [0,0,0];
key_mouth = false;
$fn = 64;
eps = 0.01;
top_floor = case_height - wall;
pcb_end = pcb_x + pcb_length;
pcb_under = board_top_z - pcb_thickness;
slot_bottom = pcb_under;
slot_top = board_top_z + pcb_slot_clearance;
flange_top = case_height - sensor_flange_depth;
flange_bottom = flange_top - sensor_flange_thickness;
cradle_top = flange_bottom - sensor_axial_play;
usb_bottom = board_top_z + usb_root_bottom_offset;
usb_top = board_top_z + usb_root_top_offset;
usb_port_bottom = usb_bottom - usb_vertical_clearance;
usb_port_top = usb_top + usb_vertical_clearance;
rear_x = pcb_end + pcb_axial_stop_gap;
rear_height = top_floor - fit_clearance;
front_ear_top = carrier_shoulder_z + front_ear_thickness;
rear_pad_depth = pad_compressed_thickness - pcb_axial_stop_gap;
top_pad_depth = pad_compressed_thickness - pcb_slot_clearance;
cradle_feet = [[sensor_x, -sensor_leg_radius],
               [sensor_x, sensor_leg_radius], [sensor_x + sensor_leg_radius, 0]];

assert(pcb_x + usb_blocking_reserve <= usb_blocking_limit,
       "USB axial blocking exceeds the 1 mm limit including reserve");
assert((boss_diameter - insert_outer_diameter)/2 >= 1.3);
assert(split_height + insert_hole_depth < top_floor);
assert(insert_hole_depth >= insert_length + 1);
assert(head_recess + screw_length < split_height + insert_length);
assert(head_recess + screw_length - split_height >= 2.0);
assert(pcb_bottom > wall && pcb_bottom + pcb_stack_height < top_floor);
assert(sensor_front_hole < sensor_diameter && sensor_front_hole > sensor_body_diameter);
assert(top_floor - usb_port_top >= 0.6);
assert(front_rib_bottom > pcb_under);
assert(front_rib_inner-front_rib_radius >= pcb_width/2+fit_clearance,
       "Outer rib fillet enters PCB clearance");
assert(front_stand_inner-carrier_root_radius >= pcb_width/2+pcb_clearance);
assert(top_floor-front_roof_radius >= usb_port_top+0.05-eps,
       "Front roof fillet enters USB clearance");
assert(wall+shell_root_radius < pcb_bottom);
assert(front_stand_depth-front_tab_root_radius-fit_clearance >= -0.000001);
assert(max(front_stand_inner+carrier_side_width+fit_clearance,
           frame_half_width+fit_clearance) <= front_width/2-front_radius,
       "Front corner relief requires a wider nose");
assert(front_tab_depth > front_stand_depth);
assert(abs(front_ear_top + rear_ear_thickness - split_height) < 0.0001);
assert((guide_diameter - screw_clearance)/2 >= insulation_wall);
assert(guide_hole_diameter > guide_diameter);
assert(pad_min_thickness > pad_compressed_thickness);
assert(rear_pad_depth > 0 && top_pad_depth > 0);
assert(abs(fixture_motion_allowance + manufacturing_reserve - usb_blocking_reserve) < 0.0001);
for (s = screws) {
    dx = max(pcb_x-s[0], 0, s[0]-pcb_end);
    dy = max(abs(s[1])-pcb_width/2, 0);
    assert(sqrt(dx*dx+dy*dy) > boss_diameter/2 + pcb_clearance);
    assert(norm(s-[sensor_x,0]) > sensor_flange_hole/2 +
           insert_outer_diameter/2 + insulation_wall);
}

module outline(inset = 0) {
    offset(delta = -inset) hull() {
        for (y = [-1, 1])
            translate([front_radius, y*(front_width/2-front_radius)]) circle(r=front_radius);
        for (y = [-1, 1])
            translate([shoulder_x, y*(case_width/2-shoulder_radius)]) circle(r=shoulder_radius);
        translate([sensor_x, 0]) circle(r=case_width/2);
    }
}

module slab(z, height, inset=0) {
    translate([0, 0, z]) linear_extrude(height) outline(inset);
}

module bosses(z, height, extra=0) {
    for (s = screws) translate([s[0], s[1], z])
        cylinder(d=boss_diameter + 2*extra, h=height);
}

module front_relief() {
    translate([-eps, -frame_half_width-fit_clearance, wall-fit_clearance])
        cube([pcb_x + fit_clearance + eps, 2*(frame_half_width+fit_clearance),
              case_height - wall + 2*fit_clearance]);
    translate([pcb_x, -pcb_width/2-pcb_clearance, pcb_bottom-0.3])
        cube([2, pcb_width+2*pcb_clearance, pcb_stack_height+0.6]);
}

module tongue(extra=0) {
    difference() {
        slab(split_height-eps, tongue_height+eps+extra, wall-tongue_width-extra);
        slab(split_height-2*eps, tongue_height+3*eps+extra, wall+extra);
        front_relief();
    }
}

module rear_foot() {
    translate([rear_x,-7,wall-0.5]) cube([2.0,14,1.4]);
}

module rear_socket() {
    translate([0,0,wall-0.6]) linear_extrude(0.6+boss_root_radius+eps)
        offset(r=fit_clearance) projection(cut=false) rear_foot();
}

module cable_fences() {
    for (y = [-1, 1])
        translate([27.8, y > 0 ? 5.8 : -6.8, wall-eps])
            cube([6.0, 1.0, 3.8+eps]);
}

module ear_clearance() {
    bosses(carrier_shoulder_z,case_height,fit_clearance);
    carrier_ear_root(fit_clearance);
}

module front_support_relief() {
    for (y=[-1,1])
        translate([pcb_x+front_stand_depth-front_tab_root_radius-fit_clearance,
                   y > 0 ? front_stand_inner-carrier_root_radius-fit_clearance :
                   -front_stand_inner-carrier_side_width-fit_clearance,
                   wall-eps])
            cube([carrier_side_width+front_tab_root_radius+2*fit_clearance,
                  carrier_side_width+carrier_root_radius+2*fit_clearance,
                  slot_bottom+fit_clearance-wall+eps]);
}

module base_shell() {
    difference() {
        union() {
            difference() {
                slab(0, split_height);
                slab(wall, split_height, wall);
                ear_clearance();
            }
            bosses(wall-eps, carrier_shoulder_z-wall+eps);
            boss_root(wall);
            perimeter_root(wall);
            front_key_holders();
            for (s = screws)
                translate([s[0],s[1],carrier_shoulder_z-eps])
                    cylinder(d=guide_diameter,
                             h=split_height-guide_top_clearance-carrier_shoulder_z+eps);
            difference() { tongue(); ear_clearance(); }
            cable_fences();
            sensor_support();
        }
        for (s = screws) {
            translate([s[0], s[1], -eps]) cylinder(d=screw_clearance, h=split_height+2*eps);
            translate([s[0], s[1], -eps]) cylinder(d=head_diameter, h=head_recess+eps);
        }
        front_relief();
        front_support_relief();
        carrier_ear_root(fit_clearance);
        rear_socket();
        front_key_cavities();
    }
}

module base() {
    union() {
        base_shell();
        translate([0,-usb_width/2+fit_clearance,0])
            cube([pcb_x,usb_width-2*fit_clearance,usb_port_bottom]);
        root_strip([pcb_x,-usb_width/2+fit_clearance,wall-fit_clearance],
                   [1,0,0], [0,0,1], [0,1,0],usb_width-2*fit_clearance,shell_root_radius);
    }
}

module lid_shell() {
    difference() {
        union() {
            difference() {
                slab(split_height+lid_perimeter_gap, case_height-split_height-lid_perimeter_gap);
                slab(split_height-eps, top_floor-split_height+eps, wall);
            }
            bosses(split_height, top_floor-split_height+eps);
            boss_root(top_floor,true);
            perimeter_root(top_floor,true);
            for (y = [-1, 1])
                translate([5, y > 0 ? 9.9 : -10.9, 15.0])
                    cube([screws[0][0]-5, 1.0, top_floor-15.0+eps]);
            // This lip bears on the flange, leaving the sensing surface uncovered.
            translate([sensor_x, 0, flange_top])
                cylinder(d=sensor_lip_outer, h=case_height-flange_top);
        }
        tongue(fit_clearance);
        for (s = screws)
            translate([s[0], s[1], split_height-eps])
                cylinder(d=insert_hole_diameter, h=insert_hole_depth+eps);
        translate([sensor_x, 0, flange_top-eps])
            cylinder(d=sensor_front_hole, h=case_height-flange_top+2*eps);
        translate([sensor_x, 0, case_height-0.3])
            cylinder(d1=sensor_front_hole, d2=sensor_front_hole+0.6, h=0.3+eps);
        translate([sensor_x, 0, split_height-eps])
            cylinder(d=sensor_flange_hole, h=flange_top-split_height+eps);
        translate([sensor_x,0,flange_top-eps])
            cylinder(d=sensor_pad_groove_outer, h=pad_compressed_thickness+eps);
        for (p = controls) translate([p[0], p[1], top_floor-eps])
            cylinder(d=3.2, h=wall+2*eps);
        translate([led_xy[0], led_xy[1], top_floor-eps]) cylinder(d=2, h=wall+2*eps);
        front_relief();
        front_support_relief();
        // The front roof meets the thin wall through its own continuous blend.
        translate([-eps,-frame_half_width,pcb_bottom+pcb_stack_height+0.3-eps])
            cube([pcb_x+2+eps,2*frame_half_width,
                  top_floor-(pcb_bottom+pcb_stack_height+0.3)+eps]);
    }
}

module front_upper_ribs() {
    for (y=[-1,1])
        root_gusset(y*front_tab_y,front_tab_width,
                    [[-eps,slot_top+front_upper_root_rise],
                     [front_upper_toe_start,slot_top], [front_upper_toe_depth,slot_top],
                     [front_bearing_rib_length,top_floor+eps],[-eps,top_floor+eps]],
                    front_upper_radius,slot_top);
}

module lid_front() {
    intersection() {
      slab(0,case_height);
      difference() {
        union() {
            translate([0, -frame_half_width, wall+0.1])
                cube([pcb_x, 2*frame_half_width, case_height-wall-0.1]);
            translate([0,-frame_half_width,top_floor])
                cube([pcb_x+fit_clearance+2*eps,2*frame_half_width,wall]);
            for (y = [-1, 1]) {
                root_gusset(y*(front_rib_inner+front_rib_width/2),front_rib_width,
                            [[-eps,front_rib_bottom],[-eps,top_floor+eps],
                             [front_rib_length,top_floor+eps]],
                            front_rib_radius,slot_bottom+fit_clearance+0.1);
            }
            front_upper_ribs();
            front_roof_root();
        }
        translate([-eps, 0, (usb_port_top+usb_port_bottom)/2]) rotate([0,90,0])
            linear_extrude(pcb_x+2*eps) offset(r=0.4)
                square([usb_port_top-usb_port_bottom-0.8,usb_width-0.8], center=true);
        // The open bottom lets the lid descend around an already fitted USB plug.
        translate([-eps,-usb_width/2,wall-eps])
            cube([pcb_x+2*eps,usb_width,usb_port_bottom+0.4-wall+2*eps]);
      }
    }
}

module lid() { union() { lid_shell(); lid_front(); front_keys(); } }

module carrier_v10_body() {
    rail_end = screws[0][0]-boss_diameter/2-fit_clearance;
    difference() {
        union() {
            for (y = [-1,1]) {
                translate([pcb_x+front_tab_depth-eps,
                           y > 0 ? carrier_side_inner : -carrier_side_inner-carrier_side_width,3.0])
                    cube([rail_end-pcb_x-front_tab_depth+eps,
                          carrier_side_width,front_ear_top-3.0]);
                translate([rail_end-eps,
                           y > 0 ? carrier_side_inner : -carrier_side_inner-carrier_side_width,
                           carrier_shoulder_z])
                    cube([screws[0][0]-rail_end+eps,carrier_side_width,front_ear_thickness]);
                translate([pcb_x+front_stand_depth,
                           y > 0 ? front_stand_inner : -front_stand_inner-carrier_side_width,3.0])
                    cube([carrier_side_width,carrier_side_width,slot_bottom-3.0]);
                translate([pcb_x, y > 0 ? front_tab_y-front_tab_width/2 : -front_stand_inner-carrier_side_width,
                           slot_bottom-1.0])
                    cube([front_tab_depth,front_stand_inner+carrier_side_width-front_tab_y+front_tab_width/2,1.0]);
            }
            carrier_front_roots();
            carrier_ear_root();
            bosses(carrier_shoulder_z,front_ear_thickness+rear_ear_thickness);
            rear_foot();
            for (y = [-1, 1]) {
                translate([rear_x, y*rear_stand_y-rear_stand_width/2, wall])
                    cube([1.6, rear_stand_width, rear_height-wall]);
                translate([pcb_end-rear_tab_depth, y > 0 ? 3.8 : -6.3, slot_bottom-1])
                    cube([rear_x+1.6-pcb_end+rear_tab_depth, 2.5, 1.0]);
                hull() {
                    translate([rear_x+1.4,y*rear_stand_y,front_ear_top])
                        cylinder(d=1.6,h=rear_ear_thickness);
                    translate([screws[0][0],y*abs(screws[0][1]),front_ear_top])
                        cylinder(d=boss_diameter,h=rear_ear_thickness);
                }
            }
            translate([pcb_end-rear_tab_depth,-6.3,slot_top])
                cube([rear_x+1.6-pcb_end+rear_tab_depth,12.6,1.0]);
        }
        for (s=screws)
            translate([s[0],s[1],carrier_shoulder_z-eps])
                cylinder(d=guide_hole_diameter,h=front_ear_thickness+rear_ear_thickness+2*eps);
        for (y=[-1,1])
            translate([rear_x-eps,y*rear_stand_y-rear_pad_pocket_width/2,
                       (pcb_under+board_top_z-rear_pad_pocket_height)/2])
                cube([rear_pad_depth+eps,rear_pad_pocket_width,rear_pad_pocket_height]);
        translate([pcb_end-rear_tab_depth,-6.3,board_top_z-eps])
            cube([top_pad_length+0.4,12.6,pad_compressed_thickness+eps]);
    }
}

module carrier(side) {
    intersection() {
        carrier_body();
        translate([-eps,side > 0 ? carrier_split_gap/2 : -case_width,0])
            cube([sensor_x,case_width-carrier_split_gap/2,case_height]);
    }
}

module carriers() { carrier(-1); carrier(1); }

module sensor_support() {
    difference() {
        union() {
            difference() {
                translate([sensor_x, 0, cradle_top-1.2])
                    cylinder(d=sensor_cradle_outer, h=1.2);
                translate([sensor_x, 0, cradle_top-1.2-eps])
                    cylinder(d=sensor_front_hole, h=1.2+2*eps);
            }
            for (i=[0:2]) {
                p=cradle_feet[i];
                translate([p[0]-(i==2?sensor_leg_width:sensor_leg_tangent)/2,
                           p[1]-(i==2?sensor_leg_tangent:sensor_leg_width)/2,wall-eps])
                    cube([i==2?sensor_leg_width:sensor_leg_tangent,
                          i==2?sensor_leg_tangent:sensor_leg_width,cradle_top-wall+eps]);
            }
        }
        bosses(0, case_height, fit_clearance);
    }
}

module pads_installed() {
    for (y=[-1,1])
        translate([pcb_end,y*rear_stand_y-rear_pad_width/2,
                   (pcb_under+board_top_z-rear_pad_height)/2])
            cube([pad_compressed_thickness,rear_pad_width,rear_pad_height]);
    translate([pcb_end-rear_tab_depth,-top_pad_width/2,board_top_z])
        cube([top_pad_length,top_pad_width,pad_compressed_thickness]);
    translate([sensor_x,0,flange_top]) difference() {
        cylinder(d=sensor_pad_installed_envelope,h=pad_compressed_thickness);
        translate([0,0,-eps]) cylinder(d=sensor_body_diameter,h=pad_compressed_thickness+2*eps);
    }
}

module sensor_reference() {
    translate([sensor_x, 0, case_height-5.0]) cylinder(d=sensor_body_diameter, h=5.0);
    translate([sensor_x, 0, flange_bottom])
        cylinder(d=sensor_diameter, h=sensor_flange_thickness);
    translate([sensor_x-1.5, -4, case_height-sensor_height]) cube([3, 8, 1.3]);
}

module usb_reference() {
    // A rectangular clearance envelope, not a reconstruction of the connector.
    translate([pcb_x-usb_protrusion,-usb_reference_width/2,usb_bottom])
        cube([usb_protrusion,usb_reference_width,usb_top-usb_bottom]);
}

module board_reference() {
    // A conservative component check avoids the non-closed assembly STL.
    for (b = official_components)
        translate([pcb_end-b[4], b[0]-pcb_width/2, board_top_z+b[2]])
            cube([b[4]-b[1], b[3]-b[0], b[5]-b[2]]);
    translate([pcb_x+1,-pcb_width/2+1,pcb_under]) linear_extrude(pcb_thickness)
        offset(r=1) square([pcb_length-2,pcb_width-2]);
}

module hardware() {
    for (s = screws) {
        translate([s[0], s[1], head_recess-screw_head_height])
            cylinder(d=screw_head_outer, h=screw_head_height);
        translate([s[0], s[1], head_recess]) cylinder(d=2, h=screw_length);
        translate([s[0], s[1], split_height])
            cylinder(d=insert_outer_diameter, h=insert_length);
    }
}

module plastic_piece(i) {
    if (i == 0) base();
    else if (i == 1) lid();
    else if (i == 2) carrier(-1);
    else if (i == 3) carrier(1);
    else assert(false, "Unknown piece");
}

module plastics() { for (i = [0:3]) plastic_piece(i); }

module cable_corridor() {
    translate([pcb_end,-4.4,3.0]) cube([sensor_x+1.5-pcb_end,8.8,7.5]);
    translate([27,-4.4,10.5]) cube([sensor_x+1.5-27,8.8,1.9]);
}

module stack_except_bearing_patches() {
    difference() {
        translate([pcb_x,-pcb_width/2,pcb_bottom])
            cube([pcb_length,pcb_width,pcb_stack_height]);
        for (y = [-1,1]) {
            translate([pcb_x-eps,
                       y > 0 ? front_tab_y-front_tab_width/2-eps : -carrier_side_inner-carrier_side_width-eps,
                       slot_bottom-1-eps])
                cube([front_tab_depth+2*eps,
                      carrier_side_inner+carrier_side_width-front_tab_y+front_tab_width/2+2*eps,
                      1+2*eps]);
            translate([pcb_x-eps,y*front_tab_y-front_tab_width/2-eps,slot_top-eps])
                cube([front_tab_depth+2*eps,front_tab_width+2*eps,1+2*eps]);
            translate([pcb_end-rear_tab_depth-eps, y > 0 ? 3.8-eps : -6.3-eps,
                       slot_bottom-1-eps])
                cube([rear_tab_depth+2*eps,2.5+2*eps,slot_top-slot_bottom+2+2*eps]);
        }
        translate([pcb_end-rear_tab_depth-eps,-6.3-eps,slot_top-eps])
            cube([rear_tab_depth+2*eps,12.6+2*eps,1+2*eps]);
        // The official component check independently covers these upper front ribs.
        front_upper_ribs();
    }
}

module assembly(explode=0) {
    color([0.22,0.25,0.29]) base();
    color([0.55,0.60,0.65]) translate([0,0,explode]) lid();
    color([0.15,0.55,0.63]) translate([0,0,explode/3]) carriers();
    color([0.75,0.48,0.25]) translate([0,0,explode/2]) pads_installed();
    color([0.12,0.3,0.5]) translate([0,0,explode/2]) board_reference();
    color([0.75,0.78,0.8]) translate([0,0,explode/2]) sensor_reference();
    color([0.7,0.73,0.77]) translate([0,0,explode/2]) usb_reference();
}

if (part == "base") base();
else if (part == "lid") lid();
else if (part == "carrier-left") carrier(-1);
else if (part == "carrier-right") carrier(1);
else if (part == "carrier-v10-left") carrier_v10(-1);
else if (part == "carrier-core-loss") difference() {
    carrier_v10(key_side); carrier(key_side);
}
else if (part == "lid-front-detail") intersection() {
    union() { lid_front(); front_keys(); }
    translate([-eps,-case_width,0])
        cube([pcb_x+front_rib_length+2*front_rib_radius+eps,2*case_width,top_floor-eps]);
}
else if (part == "pads-installed") pads_installed();
else if (part == "assembled") assembly();
else if (part == "exploded") assembly(24);
else if (part == "plastic") plastics();
else if (part == "sensor") sensor_reference();
else if (part == "usb") usb_reference();
else if (part == "board") board_reference();
else if (part == "hardware") hardware();
else if (part == "section") intersection() {
    assembly(); translate([-10,0,0]) cube([80,30,60]);
}
else if (part == "pair-collision") intersection() {
    plastic_piece(pair[0]); translate(pair_offset) plastic_piece(pair[1]);
}
else if (part == "board-collision") intersection() {
    plastics(); translate(collision_shift) board_reference();
}
else if (part == "retention-regression") intersection() {
    union() {
        base(); lid(); translate([0.05,0,0]) carriers();
    }
    translate([0.25,0,0]) union() { board_reference(); usb_reference(); }
}
else if (part == "sensor-collision") intersection() {
    plastics(); translate(collision_shift) sensor_reference();
}
else if (part == "usb-collision") intersection() {
    plastics(); translate(collision_shift) usb_reference();
}
else if (part == "pad-plastic-collision") intersection() { plastics(); pads_installed(); }
else if (part == "metal-collision") intersection() {
    translate(collision_shift) hardware();
    union() {
        translate([pcb_x-insulation_wall,-pcb_width/2-insulation_wall,
                   pcb_bottom-insulation_wall])
            cube([pcb_length+2*insulation_wall, pcb_width+2*insulation_wall,
                  pcb_stack_height+2*insulation_wall]);
        translate([sensor_x,0,case_height-sensor_height-insulation_wall])
            cylinder(d=sensor_diameter+2*insulation_wall,
                     h=sensor_height+2*insulation_wall);
    }
}
else if (part == "cable-collision") intersection() { plastics(); cable_corridor(); }
else if (part == "full-stack-collision") intersection() {
    plastics(); translate(collision_shift) stack_except_bearing_patches();
}
else if (part == "lower-stack-collision") intersection() {
    plastics();
    translate([pcb_x,-pcb_width/2,pcb_bottom]+collision_shift)
        cube([pcb_length,pcb_width,slot_bottom-1.0-pcb_bottom]);
}
else if (part == "loading-collision") intersection() {
    base();
    translate([0,0,assembly_lift]) union() {
        board_reference(); usb_reference(); carriers();
        translate([pcb_x,-pcb_width/2,pcb_bottom])
            cube([pcb_length,pcb_width,slot_bottom-1.0-pcb_bottom]);
    }
}
else if (part == "loading-sweep") intersection() {
    base();
    minkowski() {
        union() {
            carriers();
            usb_reference();
            translate([pcb_x,-pcb_width/2,pcb_bottom])
                cube([pcb_length,pcb_width,slot_bottom-1.0-pcb_bottom]);
        }
        // Keep sweep padding behind the intended PCB/front-wall contact plane.
        translate([0,-0.001,0]) cube([0.002,0.002,20]);
    }
}
else if (part == "lid-closing-sweep") intersection() {
    union() {
        base(); carriers();
        board_reference(); sensor_reference(); usb_reference();
    }
    minkowski() {
        lid(); translate([-0.002,-0.001,0]) cube([0.002,0.002,20]);
    }
}
else if (part == "carrier-side-loading-collision") intersection() {
    union() {
        board_reference(); usb_reference();
        translate([pcb_x,-pcb_width/2,pcb_bottom])
            cube([pcb_length,pcb_width,slot_bottom-1.0-pcb_bottom]);
    }
    for (side=[-1,1])
        translate([0,side*assembly_side_offset,0]+collision_shift) carrier(side);
}
else if (part == "front-rib-section") intersection() {
    lid();
    translate([pcb_x+2.0,front_rib_inner+0.1,front_rib_bottom+2.6])
        cube([0.2,front_rib_width-0.2,0.2]);
}
else if (part == "front-bearing-rib-section") intersection() {
    lid();
    translate([pcb_x+1.5,front_tab_y-front_tab_width/2+0.1,slot_top+2.0])
        cube([0.2,front_tab_width-0.2,0.2]);
}
else if (part == "root-probe") intersection() {
    plastic_piece(probe_piece);
    translate(probe_point) cube([0.04,0.04,0.04],center=true);
}
else if (part == "web-core-loss") difference() {
    v7_rib_core(web_kind);
    lid();
}
else if (part == "front-key-collision") front_key_collision();
else if (part == "front-key-wrong-contact") difference() {
    front_key_collision(); front_key_contact_window();
}
else if (part == "front-key-skin-loss") linear_extrude(1) front_key_skin_loss(key_mouth);
else if (part == "front-key-base" || part == "front-key-lid") intersection() {
    if (part == "front-key-base") base(); else lid();
    translate([-eps,key_holder_inner-0.5,0])
        cube([key_holder_end+0.5+eps,front_width,key_top+1]);
}
else assert(false, "Unknown part");

'''

MODEL += FILLETS + RIB_REFERENCE + FRONT_KEYS + CARRIERS
