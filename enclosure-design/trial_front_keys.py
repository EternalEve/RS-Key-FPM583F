# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Rigid vertical front keys and load-bearing sockets, assembled without flexing."""

FRONT_KEYS = r'''
key_top = front_key_z + front_key_height;
key_back_gap = 2*fit_clearance-front_key_pull_gap;
key_holder_x = front_key_head_x-front_key_pull_gap-front_key_shoulder;
key_holder_end = front_key_head_x+front_key_head_depth+key_back_gap+front_key_outer_wall;
key_holder_inner = front_key_y-front_key_head_width/2-fit_clearance-front_key_inner_wall;
key_holder_outer = front_key_y+front_key_head_width/2+fit_clearance+front_key_outer_wall;

assert(front_key_pull_gap > 0 && front_key_pull_gap <= fixture_motion_allowance,
       "Key pull gap exceeds motion allowance");
assert(front_key_head_width > front_key_width+2*front_key_blend_radius);
assert(front_key_z >= wall+fit_clearance-eps);
assert(key_top <= front_key_socket_top-front_key_lead+eps);
assert(front_key_socket_top < split_height-fit_clearance);
assert(key_holder_inner >= carrier_side_inner+carrier_side_width+fit_clearance-eps,
       "Key holder enters carrier clearance");
assert(key_holder_x >= pcb_x+front_stand_depth+carrier_side_width+fit_clearance,
       "Key shoulder enters front carrier pocket");
assert(front_key_y+front_key_width/2+front_key_root_radius <= frame_half_width+eps);

module front_key_head_profile() {
    translate([front_key_head_x+front_key_corner_radius,
               front_key_y-front_key_head_width/2+front_key_corner_radius])
        offset(r=front_key_corner_radius)
            square([front_key_head_depth-2*front_key_corner_radius,
                    front_key_head_width-2*front_key_corner_radius]);
}

module front_key_profile() {
    union() {
        translate([pcb_x-eps,front_key_y-front_key_width/2])
            square([front_key_head_x-pcb_x+2*eps,front_key_width]);
        front_key_head_profile();
        for (side=[-1,1])
            if (front_key_blend_radius > 0)
            translate([front_key_head_x,front_key_y+side*front_key_width/2])
                scale([-1,side]) root_profile(front_key_blend_radius);
    }
}

module front_key_post_profile() {
    union() {
        translate([pcb_x-eps,front_key_y-front_key_width/2])
            square([front_key_post_depth+eps,front_key_width]);
        for (side=[-1,1])
            if (front_key_root_radius > 0)
            translate([pcb_x,front_key_y+side*front_key_width/2])
                scale([1,side]) root_profile(front_key_root_radius);
    }
}

module front_key(side) {
    if (front_keys_enabled) scale([1,side,1]) union() {
        translate([0,0,front_key_z]) linear_extrude(front_key_height) front_key_profile();
        translate([0,0,front_key_z]) linear_extrude(top_floor-front_key_z+eps)
            front_key_post_profile();
        root_strip([pcb_x+front_key_post_depth,front_key_y-front_key_width/2,key_top],
                   [1,0,0],[0,0,1],[0,1,0],front_key_width,front_key_transition_radius);
        root_strip([pcb_x+front_key_post_depth,front_key_y-front_key_width/2,top_floor],
                   [1,0,0],[0,0,-1],[0,1,0],front_key_width,front_key_root_radius);
        for (edge=[-1,1])
            root_strip([pcb_x,front_key_y+edge*front_key_width/2,top_floor],
                       [0,edge,0],[0,0,-1],[1,0,0],front_key_post_depth,front_key_root_radius);
    }
}

module front_keys() { for (side=[-1,1]) front_key(side); }

module front_key_socket_profile(head_only=false) {
    // Offset the profile toward the rear to make the pull-direction gap smaller.
    translate([fit_clearance-front_key_pull_gap,0]) offset(r=fit_clearance)
        if (head_only) front_key_head_profile(); else front_key_profile();
}

module front_key_holders() {
    if (front_key_shoulders) intersection() {
        slab(0,case_height);
        for (side=[-1,1]) scale([1,side,1]) union() {
            translate([key_holder_x,key_holder_inner,wall-eps])
                cube([key_holder_end-key_holder_x,key_holder_outer-key_holder_inner,
                      front_key_socket_top-wall+eps]);
            root_strip([key_holder_x,key_holder_inner,wall],[-1,0,0],[0,0,1],[0,1,0],
                       key_holder_outer-key_holder_inner,shell_root_radius);
            root_strip([key_holder_x,key_holder_inner,wall],[0,-1,0],[0,0,1],[1,0,0],
                       key_holder_end-key_holder_x,shell_root_radius);
            root_strip([key_holder_end,key_holder_inner,wall],[1,0,0],[0,0,1],[0,1,0],
                       key_holder_outer-key_holder_inner,shell_root_radius);
        }
    }
}

module front_key_cavities() {
    for (side=[-1,1]) scale([1,side,1]) {
        // The head must also pass the taller shell wall and the mating tongue.
        translate([0,0,wall]) linear_extrude(case_height)
            front_key_socket_profile();
        for (i=[0:8])
            translate([0,0,front_key_socket_top-front_key_lead+i*front_key_lead/8])
                linear_extrude(front_key_lead/8+eps)
                    offset(r=i*front_key_lead/8) front_key_socket_profile();
        translate([0,0,wall]) linear_extrude(case_height)
            offset(r=fit_clearance) front_key_post_profile();
    }
}

module front_key_collision() {
    intersection() { base(); translate(key_shift) front_key(key_side); }
}

module front_key_contact_window() {
    scale([1,key_side,1])
        translate([front_key_head_x-front_key_blend_radius+key_shift[0]-eps,
                   front_key_y-front_key_head_width/2-eps,front_key_z-eps])
            cube([front_key_blend_radius-key_shift[0]+eps,
                  front_key_head_width+2*eps,front_key_height+2*eps]);
}

module front_key_skin_loss(mouth=false) {
    difference() {
        if (mouth) offset(r=front_key_lead) front_key_socket_profile(true);
        else front_key_socket_profile(true);
        outline(front_key_outer_wall-(mouth ? front_key_lead : 0));
    }
}
'''
