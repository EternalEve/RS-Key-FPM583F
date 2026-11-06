# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""V16 local mounting bulges and carriers with thicker manufacturing sections."""

from trial_carriers import CARRIERS
from trial_fillets import FILLETS
from redesign_load_roots import LOAD_ROOTS
from thin_wall_checks import WALLS
from redesign_perimeter import PERIMETER_ROOTS


MODEL = r'''
part = "exploded";
collision_shift = [0,0,0];
pair = [0,1];
assembly_lift = 0;
assembly_side_offset = 0;
usb_root_side = -1;
root_piece = 0;
root_probe_center = [0,0,0];
wall_side = 1;
wall_kind = "ear";
$fn = 64;
eps = 0.01;
top_floor = case_height-wall;
base_top = top_floor-lid_seam_gap;
pcb_end = pcb_x+pcb_length;
pcb_under = board_top_z-pcb_thickness;
slot_bottom = pcb_under;
slot_top = board_top_z+pcb_slot_clearance;
flange_top = case_height-sensor_flange_depth;
flange_bottom = flange_top-sensor_flange_thickness;
cradle_top = flange_bottom-sensor_axial_play;
usb_bottom = board_top_z+usb_root_bottom_offset;
usb_top = board_top_z+usb_root_top_offset;
usb_port_bottom = usb_bottom-usb_vertical_clearance;
usb_port_top = usb_top+usb_vertical_clearance;
rear_x = pcb_end+pcb_axial_stop_gap;
rear_height = top_floor-fit_clearance;
front_ear_top = carrier_shoulder_z+front_ear_thickness;
rear_pad_depth = pad_compressed_thickness-pcb_axial_stop_gap;
key_holder_end = front_key_head_x+front_key_head_depth+
                 2*fit_clearance-front_key_pull_gap+front_key_outer_wall;

assert(pcb_x+usb_blocking_reserve <= usb_blocking_limit);
assert(usb_slot_bottom_radius >= 0);
assert(front_pocket_half <= front_width/2-front_radius);
assert(front_stand_inner+carrier_front_width+fit_clearance <= front_pocket_half);
assert(carrier_side_inner+carrier_beam_width+fit_clearance <= body_width/2-wall);
assert(top_floor-usb_port_top >= 0.6);
assert(split_height+insert_hole_depth < top_floor);
assert(head_recess+screw_length-split_height >= 2);
assert(head_recess+screw_length < split_height+insert_length);

module original_outline() {
    hull() {
        for (s=[-1,1]) {
            translate([front_radius,s*(front_width/2-front_radius)]) circle(r=front_radius);
            translate([shoulder_x,s*(body_width/2-shoulder_radius)]) circle(r=shoulder_radius);
        }
        translate([sensor_x,0]) circle(r=case_width/2);
    }
}
module expanded_outline() {
    union() {
        original_outline();
        if (mount_bulges) for (s=screws) translate(s)
            circle(r=carrier_ear_diameter/2+fit_clearance+mount_shell_wall);
    }
}
module outline(inset=0) {
    offset(delta=-inset) union() {
        expanded_outline();
        if (mount_bulges) intersection() {
            offset(r=-mount_blend_radius) offset(r=mount_blend_radius) expanded_outline();
            for (s=screws) translate(s)
                circle(r=carrier_ear_diameter/2+fit_clearance+mount_shell_wall+mount_blend_radius);
        }
    }
}

module slab(z,h,inset=0) { translate([0,0,z]) linear_extrude(h) outline(inset); }
module bosses(z,h,extra=0) {
    for (s=screws) translate([s[0],s[1],z]) cylinder(d=boss_diameter+2*extra,h=h);
}

module rear_foot() {
    translate([rear_x,-7,wall-0.5]) cube([2,14,1.4]);
    difference() {
        translate([rear_x,-rear_foot_half_width,wall-0.5])
            cube([rear_post_depth,2*rear_foot_half_width,1.4]);
        bosses(0,case_height,fit_clearance);
    }
}
module rear_socket() {
    translate([0,0,wall-0.6]) linear_extrude(0.6+boss_root_radius+eps)
        offset(r=fit_clearance) projection(cut=false) rear_foot();
}
module ear_clearance() {
    ear_envelope(carrier_shoulder_z,case_height,fit_clearance);
    carrier_ear_root(fit_clearance);
}
module ear_envelope(z,h,extra=0) {
    for (s=screws) translate([s[0],s[1],z]) cylinder(d=carrier_ear_diameter+2*extra,h=h);
}

module usb_slot_profile(radius) {
    if (radius==0)
        translate([-usb_width/2,usb_port_bottom]) square([usb_width,case_height]);
    else union() {
        translate([-usb_width/2+radius,usb_port_bottom])
            square([usb_width-2*radius,case_height]);
        translate([-usb_width/2,usb_port_bottom+radius]) square([usb_width,case_height]);
        for (s=[-1,1]) translate([s*(usb_width/2-radius),usb_port_bottom+radius])
            circle(r=radius);
    }
}
module usb_front_extrusion() {
    translate([-eps,0,0]) rotate([90,0,90]) linear_extrude(pcb_x+2*eps) children();
}

module base(usb_radius=usb_slot_bottom_radius, roots=load_roots_enabled) {
    difference() {
        intersection() {
            slab(0,case_height);
            union() {
                difference() {
                    slab(0,base_top);
                    slab(wall,case_height,wall);
                    translate([pcb_x,-front_pocket_half,wall])
                        cube([front_pocket_depth-pcb_x,2*front_pocket_half,case_height]);
                    ear_clearance();
                }
                bosses(wall-eps,carrier_shoulder_z-wall+eps);
                boss_root(wall);
                intersection() {
                    base_wall_roots();
                    translate([front_pocket_depth,-body_width,0])
                        cube([sensor_x+case_width,2*body_width,case_height]);
                }
                root_strip([pcb_x,-front_pocket_half,wall],[1,0,0],[0,0,1],[0,1,0],
                           2*front_pocket_half,shell_root_radius);
                for (s=screws) translate([s[0],s[1],carrier_shoulder_z-eps])
                    cylinder(d=guide_diameter,h=split_height-guide_top_clearance-carrier_shoulder_z+eps);
                for (s=[-1,1]) translate([27.8,s>0?5.8:-6.8,wall-eps]) cube([6,1,3.8+eps]);
                sensor_support();
                if (roots) { front_corner_roots(); sensor_load_roots(); }
            }
        }
        usb_front_extrusion() usb_slot_profile(usb_radius);
        if (!front_stop_enabled)
            translate([-eps,-pcb_width/2-eps,pcb_under-eps])
                cube([pcb_x+2*eps,pcb_width+2*eps,pcb_thickness+pcb_slot_clearance+2*eps]);
        for (s=screws) {
            translate([s[0],s[1],-eps]) cylinder(d=screw_clearance,h=split_height+2*eps);
            translate([s[0],s[1],-eps]) cylinder(d=head_diameter,h=head_recess+eps);
        }
        carrier_ear_root(fit_clearance);
        rear_socket();
        translate([sensor_x,0,flange_top-fit_clearance])
            cylinder(d=sensor_lip_outer+2*fit_clearance,h=case_height-flange_top+fit_clearance+eps);
    }
}

module lid(roots=load_roots_enabled) {
    difference() {
        intersection() {
            slab(0,case_height);
            union() {
                slab(top_floor,wall);
                if (roots) lid_load_roots();
                bosses(split_height,top_floor-split_height+eps);
                intersection() { boss_root(top_floor,true); slab(0,case_height,wall+fit_clearance); }
                for (s=[-1,1]) {
                    translate([5,s>0?9.9:-10.9,15]) cube([screws[0][0]-5,1,top_floor-15+eps]);
                    translate([4,s>0?body_width/2-wall-lid_guide_gap-lid_guide_width:
                                      -body_width/2+wall+lid_guide_gap,top_floor-lid_guide_depth])
                        cube([19,lid_guide_width,lid_guide_depth+eps]);
                    translate([pcb_x,s*front_tab_y+front_tab_width/2,0]) rotate([90,0,0])
                        linear_extrude(front_tab_width)
                            polygon([[0,slot_top],[front_upper_toe_depth,slot_top],
                                     [front_bearing_rib_length,top_floor+eps],[0,top_floor+eps]]);
                }
                translate([0,-usb_width/2+fit_clearance,usb_port_top])
                    cube([pcb_x,usb_width-2*fit_clearance,case_height-usb_port_top]);
                translate([sensor_x,0,flange_top]) cylinder(d=sensor_lip_outer,h=case_height-flange_top);
            }
        }
        for (s=screws) translate([s[0],s[1],split_height-eps])
            cylinder(d=insert_hole_diameter,h=insert_hole_depth+eps);
        translate([sensor_x,0,flange_top-eps]) cylinder(d=sensor_front_hole,h=case_height-flange_top+2*eps);
        translate([sensor_x,0,case_height-0.3]) cylinder(d1=sensor_front_hole,d2=sensor_front_hole+0.6,h=0.3+eps);
        translate([sensor_x,0,split_height-eps]) cylinder(d=sensor_flange_hole,h=flange_top-split_height+eps);
        translate([sensor_x,0,flange_top-eps]) cylinder(d=sensor_pad_groove_outer,h=pad_compressed_thickness+eps);
        for (p=controls) translate([p[0],p[1],top_floor-eps]) cylinder(d=3.2,h=wall+2*eps);
        translate([led_xy[0],led_xy[1],top_floor-eps]) cylinder(d=2,h=wall+2*eps);
    }
}

module carrier_v10_body() {
    rail_end=screws[0][0]-boss_diameter/2-fit_clearance;
    difference() {
        union() {
            for (s=[-1,1]) {
                translate([pcb_x+front_tab_depth-eps,s>0?carrier_side_inner:-carrier_side_inner-carrier_side_width,3])
                    cube([rail_end-pcb_x-front_tab_depth+eps,carrier_side_width,front_ear_top-3]);
                translate([rail_end-eps,s>0?carrier_side_inner:-carrier_side_inner-carrier_side_width,carrier_shoulder_z])
                    cube([screws[0][0]-rail_end+eps,carrier_side_width,front_ear_thickness]);
                translate([pcb_x+front_stand_depth,s>0?front_stand_inner:-front_stand_inner-carrier_side_width,3])
                    cube([carrier_side_width,carrier_side_width,slot_bottom-3]);
                translate([pcb_x,s>0?front_tab_y-front_tab_width/2:-front_stand_inner-carrier_side_width,slot_bottom-1])
                    cube([front_tab_depth,front_stand_inner+carrier_side_width-front_tab_y+front_tab_width/2,1]);
            }
            carrier_front_roots();
            carrier_ear_root();
            ear_envelope(carrier_shoulder_z,front_ear_thickness+rear_ear_thickness);
            rear_foot();
            for (s=[-1,1]) {
                translate([rear_x,s*rear_stand_y-rear_stand_width/2,wall])
                    cube([rear_post_depth,rear_stand_width,slot_top+rear_top_thickness-wall]);
                translate([rear_x,s*rear_upper_y-rear_upper_width/2,wall])
                    cube([rear_post_depth,rear_upper_width,rear_height-wall]);
                translate([pcb_end-rear_tab_depth,s>0?3.8:-6.3,slot_bottom-1])
                    cube([rear_x+1.6-pcb_end+rear_tab_depth,2.5,1]);
                hull() {
                    translate([rear_x+1.4,s*rear_stand_y,front_ear_top]) cylinder(d=1.6,h=rear_ear_thickness);
                    translate([screws[0][0],s*abs(screws[0][1]),front_ear_top]) cylinder(d=carrier_ear_diameter,h=rear_ear_thickness);
                }
            }
            translate([pcb_end-rear_tab_depth,-6.3,slot_top])
                cube([rear_x+rear_post_depth-pcb_end+rear_tab_depth,12.6,rear_top_thickness]);
        }
        for (s=screws) translate([s[0],s[1],carrier_shoulder_z-eps])
            cylinder(d=guide_hole_diameter,h=front_ear_thickness+rear_ear_thickness+2*eps);
        for (s=[-1,1]) translate([rear_x-eps,s*rear_stand_y-rear_pad_pocket_width/2,
                                (pcb_under+board_top_z-rear_pad_pocket_height)/2])
            cube([rear_pad_depth+eps,rear_pad_pocket_width,rear_pad_pocket_height]);
        translate([pcb_end-rear_tab_depth,-6.3,board_top_z-eps])
            cube([top_pad_length+top_pad_end_clearance,12.6,pad_compressed_thickness+eps]);
    }
}

module carrier(side, roots=load_roots_enabled) {
    intersection() {
        carrier_body() if (roots) rear_carrier_roots();
        translate([-eps,side>0?carrier_split_gap/2:-case_width,0])
            cube([sensor_x,case_width-carrier_split_gap/2,case_height]);
    }
}
module carriers() { carrier(-1); carrier(1); }
module sensor_support() {
    difference() {
        union() {
            translate([sensor_x,0,cradle_top-1.2]) difference() {
                cylinder(d=sensor_cradle_outer,h=1.2);
                translate([0,0,-eps]) cylinder(d=sensor_front_hole,h=1.2+2*eps);
            }
            for (i=[0:2]) {
                p=i==0?[sensor_x,-sensor_leg_radius]:i==1?[sensor_x,sensor_leg_radius]:[sensor_x+sensor_leg_radius,0];
                translate([p[0]-(i==2?sensor_leg_width:sensor_leg_tangent)/2,
                           p[1]-(i==2?sensor_leg_tangent:sensor_leg_width)/2,wall-eps])
                    cube([i==2?sensor_leg_width:sensor_leg_tangent,i==2?sensor_leg_tangent:sensor_leg_width,cradle_top-wall+eps]);
            }
        }
        bosses(0,case_height,fit_clearance);
    }
}
module board_reference() {
    for (b=official_components)
        translate([pcb_end-b[4],b[0]-pcb_width/2,board_top_z+b[2]])
            cube([b[4]-b[1],b[3]-b[0],b[5]-b[2]]);
    translate([pcb_x+1,-pcb_width/2+1,pcb_under]) linear_extrude(pcb_thickness)
        offset(r=1) square([pcb_length-2,pcb_width-2]);
}
module usb_reference() {
    translate([pcb_x-usb_protrusion,-usb_reference_width/2,usb_bottom])
        cube([usb_protrusion,usb_reference_width,usb_top-usb_bottom]);
}
module sensor_reference() {
    translate([sensor_x,0,case_height-5]) cylinder(d=sensor_body_diameter,h=5);
    translate([sensor_x,0,flange_bottom]) cylinder(d=sensor_diameter,h=sensor_flange_thickness);
    translate([sensor_x-1.5,-4,case_height-sensor_height]) cube([3,8,1.3]);
}
module pads_installed() {
    for (s=[-1,1]) translate([pcb_end,s*rear_stand_y-rear_pad_width/2,(pcb_under+board_top_z-rear_pad_height)/2])
        cube([pad_compressed_thickness,rear_pad_width,rear_pad_height]);
    translate([pcb_end-rear_tab_depth,-top_pad_width/2,board_top_z]) cube([top_pad_length,top_pad_width,pad_compressed_thickness]);
    translate([sensor_x,0,flange_top]) difference() {
        cylinder(d=sensor_pad_installed_envelope,h=pad_compressed_thickness);
        translate([0,0,-eps]) cylinder(d=sensor_body_diameter,h=pad_compressed_thickness+2*eps);
    }
}
module hardware() {
    for (s=screws) {
        translate([s[0],s[1],head_recess-screw_head_height]) cylinder(d=screw_head_outer,h=screw_head_height);
        translate([s[0],s[1],head_recess]) cylinder(d=2,h=screw_length);
        translate([s[0],s[1],split_height]) cylinder(d=insert_outer_diameter,h=insert_length);
    }
}
module piece(i, roots=load_roots_enabled) {
    if (i==0) base(roots=roots); else if (i==1) lid(roots); else carrier(i==2?-1:1,roots);
}
module plastics() { for (i=[0:3]) piece(i); }
module lower_stack() {
    translate([pcb_x,-pcb_width/2,pcb_bottom]) cube([pcb_length,pcb_width,slot_bottom-1-pcb_bottom]);
}
module stack_except_bearings() {
    difference() {
        translate([pcb_x,-pcb_width/2,pcb_bottom]) cube([pcb_length,pcb_width,pcb_stack_height]);
        for (s=[-1,1]) {
            translate([pcb_x-eps,s>0?front_tab_y-front_tab_width/2-eps:-carrier_side_inner-carrier_side_width-eps,slot_bottom-1-eps])
                cube([front_tab_depth+2*eps,carrier_side_inner+carrier_side_width-front_tab_y+front_tab_width/2+2*eps,1+2*eps]);
            translate([pcb_end-rear_tab_depth-eps,s>0?3.8-eps:-6.3-eps,slot_bottom-1-eps])
                cube([rear_tab_depth+2*eps,2.5+2*eps,slot_top-slot_bottom+2+2*eps]);
        }
        translate([pcb_end-rear_tab_depth-eps,-6.3-eps,slot_top-eps])
            cube([rear_tab_depth+2*eps,12.6+2*eps,rear_top_thickness+2*eps]);
        for (s=[-1,1]) translate([pcb_x,s*front_tab_y+front_tab_width/2,0]) rotate([90,0,0])
            linear_extrude(front_tab_width) polygon([[0,slot_top],[front_upper_toe_depth,slot_top],
                                                  [front_bearing_rib_length,top_floor+eps],[0,top_floor+eps]]);
    }
}
module cable_corridor() {
    translate([pcb_end,-4.4,3]) cube([sensor_x+1.5-pcb_end,8.8,7.5]);
    translate([27,-4.4,10.5]) cube([sensor_x+1.5-27,8.8,1.9]);
}
module assembly(lift=0) {
    base(); translate([0,0,lift]) lid();
    translate([0,0,lift/2]) { carriers(); board_reference(); usb_reference(); sensor_reference(); pads_installed(); }
}
if (part=="base") base();
else if (part=="lid") lid();
else if (part=="carrier-left") carrier(-1);
else if (part=="carrier-right") carrier(1);
else if (part=="board") board_reference();
else if (part=="sensor") sensor_reference();
else if (part=="usb") usb_reference();
else if (part=="pads-installed") pads_installed();
else if (part=="hardware") hardware();
else if (part=="assembled") assembly();
else if (part=="exploded") assembly(20);
else if (part=="footprint") linear_extrude(1) outline();
else if (part=="pair-collision") intersection() { piece(pair[0]); translate(collision_shift) piece(pair[1]); }
else if (part=="board-collision") intersection() { plastics(); translate(collision_shift) board_reference(); }
else if (part=="base-board-collision") intersection() { base(); translate(collision_shift) board_reference(); }
else if (part=="stack-collision") intersection() { plastics(); translate(collision_shift) stack_except_bearings(); }
else if (part=="sensor-collision") intersection() { plastics(); translate(collision_shift) sensor_reference(); }
else if (part=="usb-collision") intersection() { plastics(); translate(collision_shift) usb_reference(); }
else if (part=="usb-clearance-collision") intersection() {
    plastics(); translate(collision_shift) usb_front_extrusion()
        offset(r=usb_vertical_clearance)
            translate([-usb_reference_width/2,usb_bottom])
                square([usb_reference_width,usb_top-usb_bottom]);
}
else if (part=="usb-slot-material-loss") difference() { base(0); base(); }
else if (part=="usb-slot-root-material") intersection() {
    difference() { base(); base(0); }
    translate([pcb_x/2,usb_root_side*(usb_width/2-0.05),usb_port_bottom+0.05])
        cube([0.04,0.04,0.04],center=true);
}
else if (part=="usb-slot-root-air") intersection() {
    base();
    for (s=[-1,1]) translate([pcb_x/2,s*(usb_width/2-0.2),usb_port_bottom+0.2])
        cube([0.02,0.02,0.02],center=true);
}
else if (part=="load-root-material-loss") difference() { piece(root_piece,false); piece(root_piece); }
else if (part=="load-root-core") piece(root_piece,false);
else if (part=="load-root-added-material") difference() { piece(root_piece); piece(root_piece,false); }
else if (part=="load-root-probe") intersection() {
    difference() { piece(root_piece); piece(root_piece,false); }
    translate(root_probe_center) cube([0.04,0.04,0.04],center=true);
}
else if (part=="front-corner-clearance") intersection() {
    intersection() {
        front_corner_roots();
        difference() { base(); base(roots=false); }
    }
    minkowski() {
        carriers();
        translate([-fit_clearance,-fit_clearance,-fit_clearance])
            cube([2*fit_clearance,2*fit_clearance,20+2*fit_clearance]);
    }
}
else if (part=="minimum-wall-missing") missing_wall();
else if (part=="floor-root-loss") difference() {
    intersection() {
        difference() { original_wall_root(); mount_root_window(2*eps); }
        linear_extrude(case_height) offset(delta=-wall) original_outline();
        translate([front_pocket_depth,-case_width,0]) cube([case_height*3,2*case_width,case_height]);
    }
    base();
}
else if (part=="pad-collision") intersection() { plastics(); pads_installed(); }
else if (part=="cable-collision") intersection() { plastics(); cable_corridor(); }
else if (part=="metal-collision") intersection() {
    translate(collision_shift) hardware();
    union() {
        translate([pcb_x-insulation_wall,-pcb_width/2-insulation_wall,pcb_bottom-insulation_wall])
            cube([pcb_length+2*insulation_wall,pcb_width+2*insulation_wall,pcb_stack_height+2*insulation_wall]);
        translate([sensor_x,0,case_height-sensor_height-insulation_wall])
            cylinder(d=sensor_diameter+2*insulation_wall,h=sensor_height+2*insulation_wall);
    }
}
else if (part=="loading-collision") intersection() {
    base(); translate([0,0,assembly_lift]) { carriers(); board_reference(); usb_reference(); lower_stack(); }
}
else if (part=="loading-sweep") intersection() {
    base(); minkowski() {
        union() { carriers(); board_reference(); usb_reference(); lower_stack(); }
        translate([0,-0.001,0]) cube([0.002,0.002,20]);
    }
}
else if (part=="closing-sweep") intersection() {
    union() { base(); carriers(); board_reference(); usb_reference(); sensor_reference(); }
    minkowski() { lid(); translate([0,-0.001,0]) cube([0.002,0.002,20]); }
}
else if (part=="carrier-side-collision") intersection() {
    union() { board_reference(); usb_reference(); lower_stack(); }
    for (s=[-1,1]) translate([0,s*assembly_side_offset,0]) carrier(s);
}
else assert(false,"Unknown V16 part");
'''

MODEL += FILLETS + CARRIERS + LOAD_ROOTS + WALLS + PERIMETER_ROOTS
