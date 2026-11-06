#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Draw the measured board and the still-provisional enclosure in plan/section."""

import math
from pathlib import Path
import struct

from PIL import Image, ImageDraw, ImageFont

from model import USB_PROTRUSION, USB_STANDARD_WIDTH, reference_parameters


BUILD = Path(__file__).resolve().parent / "build"
FONT = "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"
INK = "#25374a"
MUTED = "#627184"
BLUE = "#326d98"
AMBER = "#ac651b"


def main():
    p = reference_parameters()
    width = p["case_width"]
    length = p["sensor_x"] + width/2
    board_end = p["pcb_x"] + p["pcb_length"]
    board_half_width = p["pcb_width"]/2
    usb_exposed = USB_PROTRUSION - p["pcb_x"]
    image = Image.new("RGB", (1600, 1100), "#eef2f6")
    draw = ImageDraw.Draw(image)

    def text(x, y, content, size=24, color=INK):
        draw.text((x, y), content, font=ImageFont.truetype(FONT, size), fill=color)

    text(48, 30, "RS-Key / V4 主板几何核对", 38)
    text(48, 91, "官方 STEP + 用户装配尺寸 · 两颗螺丝 · 14 mm 叠层预算", 25, MUTED)
    text(1170, 58, "尺寸讨论稿 / 非加工图", 25, AMBER)
    draw.rounded_rectangle((40, 160, 1010, 680), radius=20, fill="white")
    draw.rounded_rectangle((1040, 160, 1560, 680), radius=20, fill="white")
    draw.rounded_rectangle((40, 710, 1560, 1060), radius=20, fill="white")
    text(72, 181, "01 / 去盖俯视：公版主板真实几何", 26)

    scale, origin_x, origin_y = 13.5, 145, 430

    def xy(x, y):
        return origin_x + x*scale, origin_y + y*scale

    def disk(x, y, radius, fill, outline=None, width=1):
        draw.ellipse((*xy(x-radius, y-radius), *xy(x+radius, y+radius)),
                     fill=fill, outline=outline, width=width)

    outline = []
    front_radius = p["front_radius"]
    for cx, cy, radius, start, stop in (
            (front_radius, -width/2+front_radius, front_radius, 180, 270),
            (p["sensor_x"], 0, width/2, -90, 90),
            (front_radius, width/2-front_radius, front_radius, 90, 180)):
        for degree in range(start, stop+1, 3):
            angle = math.radians(degree)
            outline.append(xy(cx + radius*math.cos(angle), cy + radius*math.sin(angle)))
    draw.polygon(outline, fill="#d9e1e8", outline="#718297", width=2)
    draw.rectangle((*xy(p["pcb_x"], -board_half_width), *xy(board_end, board_half_width)),
                   fill="#d8eaf6", outline=BLUE, width=2)
    for x, y in p["screws"]:
        disk(x, y, p["boss_diameter"]/2, "#f2dabb", AMBER, 2)
        disk(x, y, p["insert_hole_diameter"]/2, "#c6cbd0", AMBER, 2)

    data = (BUILD / "reference" / "board.stl").read_bytes()
    count = struct.unpack_from("<I", data, 80)[0]
    if len(data) != 84 + 50*count:
        raise ValueError("Expected a binary STL")
    triangles = []
    for i in range(count):
        values = struct.unpack_from("<12fH", data, 84+50*i)
        native = [values[j:j+3] for j in (3, 6, 9)]
        points = [xy(board_end-v[1], v[0]-board_half_width) for v in native]
        area = sum(points[k][0]*points[(k+1)%3][1]
                   - points[(k+1)%3][0]*points[k][1] for k in range(3))
        if abs(area) < 0.03:
            continue
        z = sum(v[2] for v in native)/3
        color = "#407b9d" if z < 0.01 else "#526575"
        if z > 1.3:
            color = "#e5e8e6"
        triangles.append((z, points, color))
    for _, points, color in sorted(triangles, key=lambda row: row[0]):
        draw.polygon(points, fill=color)

    for pos in p["controls"]:
        disk(*pos, 1.6, None, "#ec8b24", 3)
    disk(*p["led_xy"], 1.0, "#f4d057", "#9a7b1a", 2)
    disk(p["sensor_x"], 0, p["sensor_diameter"]/2, "#c8cfd3", "#8595a2", 2)
    disk(p["sensor_x"], 0, 8.5, "#26333d")
    for label, y, size in (("FPM583F", -2, 22), ("Ø21", 0.7, 21)):
        label_width = draw.textlength(label, font=ImageFont.truetype(FONT, size))
        text(*xy(p["sensor_x"]-label_width/(2*scale), y), label, size, "white")
    draw.rectangle((*xy(0, -p["usb_width"]/2), *xy(p["pcb_x"], p["usb_width"]/2)),
                   outline=AMBER, width=3)
    draw.rectangle((*xy(p["pcb_x"]-USB_PROTRUSION, -USB_STANDARD_WIDTH/2),
                    *xy(p["pcb_x"], USB_STANDARD_WIDTH/2)),
                   fill="#d1d7dc", outline=AMBER, width=2)
    text(72, 242, f"USB 名义外露 {usb_exposed:g} mm", 23, AMBER)
    draw.line((155, 278, *xy(-3, -USB_STANDARD_WIDTH/2)), fill=AMBER, width=2)
    text(350, 247, "BOOT / RUN", 22, AMBER)
    draw.line((400, 281, *xy(*p["controls"][0])), fill=AMBER, width=2)
    draw.line((*xy(0, 14.5), *xy(length, 14.5)), fill=MUTED, width=2)
    label = f"{length:g} × {width:g} mm（暂定）"
    label_width = draw.textlength(label, font=ImageFont.truetype(FONT, 23))
    text(origin_x + length*scale/2 - label_width/2, 633, label, 23, MUTED)

    text(1070, 187, "已核实", 28, BLUE)
    for i, line in enumerate(("板框、按键及 RGB 灯位置", "用户：USB 伸出板边 8 mm", "用户：裸 PCB 面至最低点 11 mm")):
        text(1070, 240+i*44, line, 23)
    text(1070, 393, "需要实物核对", 28, AMBER)
    for i, line in enumerate(("USB 中心高度、根部外廓",
                              f"轴向阻挡实测 ≤ {p['usb_blocking_limit']:g} mm",
                              "针脚、线束及模块承力台阶", "两颗螺丝布局的刚度及定位止口")):
        text(1070, 448+i*43, line, 22)

    text(72, 731, "02 / 高度预算示意：侧视，非实物叠层还原", 26)
    sx, sy, base_y = 13.5, 11.5, 1030

    def section_box(x1, z1, x2, z2, fill, outline=None, width=1):
        draw.rectangle((145+x1*sx, base_y-z2*sy, 145+x2*sx, base_y-z1*sy),
                       fill=fill, outline=outline, width=width)

    section_box(0, 0, length, p["case_height"], None, "#718297", 2)
    section_box(0, 0, length, p["wall"], "#b9c4ce")
    section_box(0, p["case_height"]-p["wall"], length, p["case_height"], "#b9c4ce")
    section_box(p["pcb_x"], p["pcb_bottom"], board_end,
                p["pcb_bottom"]+p["pcb_stack_height"], "#e3eff7", BLUE, 2)
    section_box(p["pcb_x"], p["board_top_z"]-1.6, board_end, p["board_top_z"], "#407b9d")
    key_x = p["controls"][0][0]
    section_box(key_x-1.5, p["board_top_z"], key_x+1.5, p["board_top_z"]+2.5, "#526575")
    section_box(p["sensor_x"]-p["sensor_diameter"]/2,
                p["case_height"]-p["sensor_height"],
                p["sensor_x"]+p["sensor_diameter"]/2, p["case_height"], "#c8cfd3", "#8595a2", 2)
    text(249, 947, f"{p['pcb_stack_height']:g} mm", 24, BLUE)
    text(77, 881, f"{p['case_height']:g}", 25, MUTED)
    text(72, 917, "mm", 21, MUTED)
    text(975, 801, f"按用户数据：主板 Z = {p['board_top_z']:.1f} mm", 23, BLUE)
    text(975, 849, "塑料支撑、压条与模块压环待定", 23)
    text(975, 897, f"名义阻挡 {p['pcb_x']:g} mm；预留 {p['usb_blocking_reserve']:g} mm", 22)
    text(975, 945, "已生成 6 × 26 × 19 mm 前端试插规", 23, BLUE)
    destination = BUILD / "concept-v4-review.png"
    image.save(destination)
    print(destination)


if __name__ == "__main__":
    main()
