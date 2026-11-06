# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Add material around the V10 carrier while retaining its PCB clearance envelope."""

CARRIERS = r'''
carrier_rail_end = screws[0][0]-boss_diameter/2-fit_clearance;
carrier_rear_start = key_holder_end+fit_clearance+carrier_step_radius;
carrier_toe_outer = front_stand_inner+carrier_front_width;
carrier_toe_bottom = slot_bottom-carrier_toe_root_thickness;

assert(carrier_beam_width >= carrier_side_width);
assert(carrier_front_width >= carrier_side_width);
assert(carrier_bridge_bottom >= front_key_socket_top+fit_clearance);
assert(carrier_beam_top <= split_height);
assert(carrier_toe_outer+fit_clearance <= front_key_y-front_key_width/2,
       "Carrier front pillar enters key post clearance");
assert(front_stand_inner-carrier_root_radius >= pcb_width/2+pcb_clearance);

module carrier_v10(side) {
    intersection() {
        carrier_v10_body();
        translate([-eps,side > 0 ? carrier_split_gap/2 : -case_width,0])
            cube([sensor_x,case_width-carrier_split_gap/2,case_height]);
    }
}

module carrier_front_additions() {
    difference() {
      union() {
        translate([pcb_x,front_stand_inner-carrier_root_radius,carrier_toe_bottom])
            cube([front_tab_depth,carrier_front_width+carrier_root_radius,
                  carrier_toe_root_thickness]);
        translate([pcb_x+front_stand_depth,front_stand_inner,3])
            cube([carrier_side_width,carrier_front_width,slot_bottom-3]);
        root_strip([pcb_x,front_stand_inner-carrier_root_radius,slot_bottom-1],
                   [0,-1,0],[0,0,-1],[1,0,0],front_tab_depth,carrier_step_radius);
        root_strip([pcb_x+front_stand_depth,front_stand_inner,carrier_toe_bottom],
                   [0,-1,0],[0,0,-1],[1,0,0],
                   front_tab_depth-front_stand_depth,carrier_root_radius);
        root_strip([pcb_x+front_stand_depth,front_stand_inner,carrier_toe_bottom],
                   [-1,0,0],[0,0,-1],[0,1,0],carrier_front_width,front_tab_root_radius);
      }
      // Round the setback around the existing wall; leave both shell cores intact.
      linear_extrude(case_height) offset(r=fit_clearance)
          translate([-case_width,front_stand_inner+carrier_side_width+fit_clearance])
              square([case_width+wall,case_width]);
    }
}

module carrier_reinforcements() {
    if (carrier_reinforcement) for (side=[-1,1]) scale([1,side,1]) {
        carrier_front_additions();
        intersection() {
            slab(0,case_height,wall+fit_clearance);
            union() {
                translate([pcb_x+front_tab_depth-eps,carrier_side_inner,carrier_bridge_bottom])
                    cube([screws[0][0]-pcb_x-front_tab_depth+eps,
                          carrier_beam_width,carrier_beam_top-carrier_bridge_bottom]);
                translate([carrier_rear_start,carrier_side_inner,3])
                    cube([carrier_rail_end-carrier_rear_start,carrier_beam_width,
                          carrier_beam_top-3]);
                root_strip([carrier_rear_start,carrier_side_inner,carrier_bridge_bottom],
                           [-1,0,0],[0,0,-1],[0,1,0],carrier_beam_width,carrier_step_radius);
                root_strip([carrier_rear_start,carrier_side_inner+carrier_side_width,3],
                           [-1,0,0],[0,1,0],[0,0,1],
                           carrier_bridge_bottom-3,carrier_step_radius);
            }
        }
    }
}

module carrier_body() {
    difference() {
        union() { carrier_v10_body(); carrier_reinforcements(); children(); }
        for (s=screws)
            translate([s[0],s[1],carrier_shoulder_z-eps])
                cylinder(d=guide_hole_diameter,h=split_height-carrier_shoulder_z+2*eps);
    }
}
'''
