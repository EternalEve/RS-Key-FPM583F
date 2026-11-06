# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Require actual material across the vendor-flagged carrier sections."""

WALLS = r'''
module required_wall() {
    p=[screws[0][0],wall_side*abs(screws[0][1])];
    if (wall_kind=="ear") translate([p[0],p[1],carrier_shoulder_z]) difference() {
        cylinder(d=guide_hole_diameter+2*eps+2*manufacturing_wall_min,h=split_height-carrier_shoulder_z);
        translate([0,0,-eps]) cylinder(d=guide_hole_diameter+2*eps,h=case_height);
    }
    if (wall_kind=="shell")
        translate([p[0]-0.15,wall_side*(abs(p[1])+carrier_ear_diameter/2+fit_clearance),8])
            scale([1,wall_side,1]) cube([0.3,manufacturing_wall_min,base_top-8]);
    if (wall_kind=="pad-inner" || wall_kind=="pad-outer") {
        edge=wall_kind=="pad-inner" ? -1 : 1;
        y=rear_stand_y+edge*rear_pad_pocket_width/2-(edge<0?manufacturing_wall_min:0);
        translate([rear_x,wall_side*y,(pcb_under+board_top_z-rear_pad_height)/2])
            scale([1,wall_side,1]) cube([rear_pad_depth,manufacturing_wall_min,rear_pad_height]);
    }
    if (wall_kind=="roof")
        translate([pcb_end-rear_tab_depth,wall_side*1,board_top_z+pad_compressed_thickness])
            scale([1,wall_side,1]) cube([top_pad_length,3,manufacturing_wall_min]);
    if (wall_kind=="back")
        translate([pcb_end-rear_tab_depth+top_pad_length+top_pad_end_clearance,
                   wall_side*4.8,board_top_z]) scale([1,wall_side,1])
            cube([manufacturing_wall_min,0.4,pad_compressed_thickness]);
    if (wall_kind=="foot")
        translate([rear_x,wall_side*(rear_stand_y+rear_stand_width/2+0.4),wall-0.4])
            scale([1,wall_side,1]) cube([manufacturing_wall_min,0.1,1.2]);
    if (wall_kind=="guide")
        translate([4,wall_side*(body_width/2-wall-lid_guide_gap-manufacturing_wall_min),top_floor-lid_guide_depth])
            scale([1,wall_side,1]) cube([19,manufacturing_wall_min,lid_guide_depth]);
}
module missing_wall() {
    difference() {
        required_wall();
        if (wall_kind=="shell") base(); else if (wall_kind=="guide") lid(); else carrier(wall_side);
    }
}
'''


def jobs():
    return [(f"minimum-wall-{side}-{kind}", "minimum-wall-missing",
             (f"wall_side={side}",f'wall_kind="{kind}"'), True)
            for side in (-1,1) for kind in ("ear","shell","pad-inner","pad-outer","roof","back","foot","guide")]
