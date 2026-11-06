#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Render the USB-slot roots directly from the V12 and V16 front-face meshes."""

import hashlib
import json

from PIL import Image, ImageDraw, ImageFont

from preview import read_off
from redesign_model import ROOT, parameters
from reference_preview import FONT


BUILD = ROOT / "build"


def main():
    report = json.loads((BUILD/"v16-geometry-checks.json").read_text())
    if not report["all_passed"]:
        raise RuntimeError("Candidate geometry must pass before rendering")
    values = parameters()
    image = Image.new("RGB", (1800, 1050), "#edf2f6")
    draw = ImageDraw.Draw(image)

    def text(x, y, label, size=27, color="#26394c"):
        draw.text((x, y), label, fill=color, font=ImageFont.truetype(FONT, size))

    text(50, 25, "RS-Key / V16 · USB 槽底两角优化", 42)
    text(50, 95, "R0.4 圆角保留；前壁 0.8 mm 仍需单独复审；局部最大宽度 32.1 mm", 28)
    bottom = values["board_top_z"]+values["usb_root_bottom_offset"]
    top = values["board_top_z"]+values["usb_root_top_offset"]
    port_bottom = bottom-values["usb_vertical_clearance"]
    radius = values["usb_slot_bottom_radius"]
    base_path = BUILD/"v16-base.stl"
    assert hashlib.sha256(base_path.read_bytes()).hexdigest() == report["meshes"]["base"]["stl_sha256"]

    for version, left, title in (("v12", 40, "V12 / 原直角"), ("v16", 920, "V16 / 两角 R0.4")):
        draw.rounded_rectangle((left, 165, left+840, 680), radius=22, fill="white")
        text(left+30, 185, title, 31)
        panel = Image.new("RGB", (800, 355), "white")
        painter = ImageDraw.Draw(panel)
        scale = 65

        def point(y, z):
            return (400+y*scale, 315-(z-12.2)*scale)

        _, faces = read_off(BUILD/f"{version}-base.off")
        front_faces = [face for face in faces if all(abs(v[0]) < 1e-6 for v in face)]
        if not front_faces:
            raise RuntimeError(f"No exported front face: {version}")
        for face in front_faces:
            painter.polygon([point(v[1], v[2]) for v in face], fill="#68aeb8")
        # Outline only the existing comparison rectangle; it is not a new plug model.
        y = values["usb_reference_width"]/2
        painter.rectangle((point(-y, top), point(y, bottom)), fill="#d8dde3", outline="#7b8795", width=2)
        for side in (-1, 1):
            p = point(side*(values["usb_width"]/2-0.2), port_bottom+0.2)
            painter.ellipse((p[0]-34, p[1]-34, p[0]+34, p[1]+34), outline="#ba651d", width=3)
        image.paste(panel, (left+20, 265))
        text(left+30, 627, "青色：导出前壁网格   灰色：现有 USB 参考包络", 23, "#607487")

    draw.rounded_rectangle((40, 710, 1760, 975), radius=22, fill="white")
    text(75, 735, "圆角与侧壁、槽底相切；通过补料消除尖角，保留原有承力截面。", 29)
    text(75, 793, f"R{radius:g} 后，按 8.25 × 3 mm USB 根部包络检查：最小名义净空仍为 0.25 mm。", 27)
    text(75, 847, "R0.8 对照会侵入这部分净空，因此本轮保留 R0.4。", 27)
    text(75, 900, "槽口仍朝上敞开；板件装入、合盖、螺丝及指纹模块布局保留。", 27)
    text(50, 994, "按真实 CAD 前表面绘制，视窗已裁切。未进行载荷仿真或破坏试验，不标注应力改善百分比。", 24, "#607487")
    destination = BUILD/"concept-v16-usb-detail.png"
    image.save(destination)
    print(destination)


if __name__ == "__main__":
    main()
