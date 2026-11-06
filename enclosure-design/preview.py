#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Draw a review sheet from OpenSCAD OFF meshes without an OpenGL context."""

import argparse
from math import sqrt
from pathlib import Path

from model import COMPACT_PARAMETERS, PARAMETERS, reference_parameters


BUILD = Path(__file__).resolve().parent / "build"
RIGHT = (0.954, 0.298, 0.0)
UP = (-0.164, 0.525, 0.836)
DEPTH = (0.249, -0.798, 0.55)
LIGHT = (-0.2, -0.4, 0.894)


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def read_off(path):
    lines = [line for line in path.read_text().splitlines() if line.strip()]
    header = lines.pop(0).split()
    if header[0] != "OFF":
        raise ValueError(f"Expected an OFF mesh: {path}")
    counts = header[1:] or lines.pop(0).split()
    vertex_count, face_count, _ = map(int, counts)
    vertices = [tuple(map(float, line.split())) for line in lines[:vertex_count]]
    faces = []
    for line in lines[vertex_count:vertex_count + face_count]:
        values = list(map(int, line.split()))
        if values[0] < 3 or len(values) != values[0] + 1:
            raise ValueError(f"Invalid face in {path}")
        faces.append([vertices[i] for i in values[1:]])
    return vertices, faces


def material(center, exploded, parameters):
    x, y, z = center
    sensor_radius = sqrt((x - parameters["sensor_x"]) ** 2 + y ** 2)
    if exploded and z >= 29.9:
        return (124, 133, 143)
    shift = 10.5 if exploded else 0
    if x < 0:
        return (192, 197, 204)
    sensor_bottom = parameters["case_height"] - parameters["sensor_height"]
    if sensor_radius <= parameters["sensor_diameter"]/2 + 0.1 and z >= sensor_bottom + shift:
        return ((38, 43, 48) if z >= parameters["case_height"] + shift
                else (202, 211, 215))
    if exploded and z > 12:
        pcb_end = parameters["pcb_x"] + parameters["pcb_length"]
        return (58, 134, 130) if x > pcb_end else (47, 101, 142)
    return (83, 94, 107) if exploded else (124, 133, 143)


def shade(face, rgb):
    a, b, c = face[:3]
    u = [b[i] - a[i] for i in range(3)]
    v = [c[i] - a[i] for i in range(3)]
    normal = (u[1]*v[2] - u[2]*v[1], u[2]*v[0] - u[0]*v[2],
              u[0]*v[1] - u[1]*v[0])
    length = sqrt(dot(normal, normal))
    light = abs(dot(normal, LIGHT) / length) if length else 0
    factor = 0.68 + 0.32 * light
    return "#" + "".join(f"{round(channel * factor):02x}" for channel in rgb)


def mesh_panel(filename, box, exploded=False, parameters=None, scale=None, projection=None):
    parameters = PARAMETERS if parameters is None else parameters
    right, up, depth = (RIGHT, UP, DEPTH) if projection is None else projection
    vertices, faces = read_off(BUILD / filename)
    projected = [(dot(v, right), -dot(v, up)) for v in vertices]
    low_x, low_y = (min(p[i] for p in projected) for i in (0, 1))
    high_x, high_y = (max(p[i] for p in projected) for i in (0, 1))
    left, top, width, height = box
    if scale is None:
        scale = min(width / (high_x - low_x), height / (high_y - low_y))
    origin_x = left + (width - (high_x - low_x) * scale) / 2
    origin_y = top + (height - (high_y - low_y) * scale) / 2
    ordered = []
    for face in faces:
        center = tuple(sum(p[i] for p in face) / len(face) for i in range(3))
        points = " ".join(
            f"{origin_x + (dot(p, right) - low_x)*scale:.2f},"
            f"{origin_y + (-dot(p, up) - low_y)*scale:.2f}" for p in face)
        color = shade(face, material(center, exploded, parameters))
        ordered.append((dot(center, depth),
                        f'<polygon points="{points}" fill="{color}" '
                        f'stroke="{color}" stroke-width="0.35"/>'))
    return "\n".join(polygon for _, polygon in sorted(ordered))


def comparison_sheet():
    box_left = (72, 265, 665, 360)
    box_right = (852, 265, 665, 360)
    common_scale = 8.6
    top_projection = ((1, 0, 0), (0, 1, 0), (0, 0, 1))
    original = mesh_panel("assembled-check.off", box_left, scale=common_scale,
                          projection=top_projection)
    compact = mesh_panel("v2-assembled-check.off", box_right,
                         parameters=COMPACT_PARAMETERS, scale=common_scale,
                         projection=top_projection)
    return f'''<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->
<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="960" viewBox="0 0 1600 960">
<title>RS-Key 外壳 V1 与 V2 等比例比较</title>
<desc>V2 将螺柱移至 PCB 前端和 PCB 与指纹模块之间两侧，保持相同 PCB 叠层高度。</desc>
<rect width="1600" height="960" fill="#f3f4f5"/>
<g font-family="Noto Sans CJK SC, sans-serif" fill="#263442">
<text x="52" y="72" font-size="36" font-weight="700">RS-Key / 紧凑版 V2</text>
<text x="52" y="116" font-size="23" fill="#576675">两图等比例俯视 · 保持螺丝固定 · 保持原有 PCB 叠层</text>
<rect x="1272" y="40" width="276" height="48" rx="24" fill="#f9dfb6"/>
<text x="1410" y="72" font-size="22" text-anchor="middle" fill="#744713">讨论稿 / 非加工图</text>
<rect x="40" y="158" width="740" height="630" rx="22" fill="#ffffff"/>
<rect x="820" y="158" width="740" height="630" rx="22" fill="#ffffff"/>
<text x="72" y="207" font-size="25" font-weight="700">V1 / 原布局</text>
<text x="852" y="207" font-size="25" font-weight="700">V2 / 收窄与缩短</text>
{original}
{compact}
<text x="72" y="690" font-size="28" font-weight="700">64 × 34 × 18 mm</text>
<text x="852" y="690" font-size="28" font-weight="700">58 × 26 × 18 mm</text>
<text x="72" y="742" font-size="21" fill="#576675">螺柱沿 PCB 两侧布置</text>
<text x="852" y="742" font-size="21" fill="#576675">螺柱移到前端与模块前方两侧空隙</text>
<text x="52" y="850" font-size="24" font-weight="700">宽度减少约 24%，包络体积减少约 31%</text>
<text x="52" y="895" font-size="21" fill="#576675">壳高暂不降低，等待叠层及焊脚实测；孔位、压环和压条仍待定稿。</text>
<text x="52" y="932" font-size="20" fill="#576675">尺寸均为设计占位，不含 USB 插头；实际配合、线束和金属隔离尚未验证。</text>
</g>
</svg>
'''


def gauge_sheet():
    parameters = reference_parameters()
    geometry = mesh_panel("v4-front-gauge.off", (90, 160, 700, 550),
                          parameters=parameters,
                          projection=((0.33035, 0.943858, 0),
                                      (-0.469, 0.164, 0.868), (0.819, -0.287, 0.497)))
    return f'''<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->
<svg xmlns="http://www.w3.org/2000/svg" width="1100" height="850" viewBox="0 0 1100 850">
<title>RS-Key 前端试插规</title>
<rect width="1100" height="850" fill="#eef2f6"/>
<g font-family="Noto Sans CJK SC, sans-serif" fill="#25374a">
<text x="48" y="65" font-size="32">RS-Key / 前端试插规 · 名义阻挡 {parameters['pcb_x']:g} mm</text>
<text x="48" y="111" font-size="22" fill="#627184">6 × 26 × 19 mm · PA12 · 毫米导入 · 成品阻挡 ≤ {parameters['usb_blocking_limit']:g} mm</text>
<rect x="40" y="140" width="1020" height="570" rx="22" fill="white"/>
{geometry}
<text x="805" y="258" font-size="22">后端敞开</text>
<text x="805" y="302" font-size="22">手持定位板件</text>
<text x="805" y="410" font-size="22">前端为观察口</text>
<text x="805" y="454" font-size="22">检查 USB 露出量</text>
<text x="48" y="760" font-size="23">板件前边靠塑料定位面，PCB 上表面对齐侧面刻线。</text>
<text x="48" y="805" font-size="22" fill="#ac651b">仅供试插；不含最终小孔、夹持和螺丝结构。</text>
</g>
</svg>
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--comparison", action="store_true")
    mode.add_argument("--gauge", action="store_true")
    args = parser.parse_args()
    if args.gauge:
        destination = BUILD / "v4-front-gauge-preview.svg"
        destination.write_text(gauge_sheet(), encoding="utf-8")
        print(destination)
        return
    if args.comparison:
        destination = BUILD / "concept-comparison.svg"
        destination.write_text(comparison_sheet(), encoding="utf-8")
        print(destination)
        return
    assembled = mesh_panel("assembled-check.off", (85, 260, 650, 390))
    exploded = mesh_panel("exploded-check.off", (865, 240, 650, 455), True)
    sheet = f'''<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->
<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="960" viewBox="0 0 1600 960">
<title>RS-Key 外壳结构初稿：合壳外观与分解布局</title>
<desc>尺寸未确认的螺丝固定外壳，包含 PCB 叠层和尾端指纹模块占位。不是加工图。</desc>
<rect width="1600" height="960" fill="#f3f4f5"/>
<g font-family="Noto Sans CJK SC, sans-serif" fill="#263442">
<text x="52" y="72" font-size="36" font-weight="700">RS-Key / 外壳结构 V1</text>
<text x="52" y="116" font-size="23" fill="#576675">底部螺丝固定 · 前端 USB-C · 尾端指纹区</text>
<rect x="1272" y="40" width="276" height="48" rx="24" fill="#f9dfb6"/>
<text x="1410" y="72" font-size="22" text-anchor="middle" fill="#744713">讨论稿 / 非加工图</text>
<rect x="40" y="158" width="740" height="650" rx="22" fill="#ffffff"/>
<rect x="820" y="158" width="740" height="650" rx="22" fill="#ffffff"/>
<text x="72" y="207" font-size="25" font-weight="700">01 / 合壳外观</text>
<text x="852" y="207" font-size="25" font-weight="700">02 / 分解布局</text>
{assembled}
{exploded}
<text x="72" y="733" font-size="21">前端收窄，尾部圆润加宽</text>
<text x="72" y="770" font-size="20" fill="#576675">操作孔为占位；最终边缘将做圆滑处理</text>
<text x="852" y="733" font-size="21">上盖 / 双层 PCB 与模块占位 / 底壳</text>
<text x="852" y="770" font-size="20" fill="#576675">四处螺柱；压环与 PCB 压条待实测后设计</text>
<text x="52" y="866" font-size="23" font-weight="700">64 × 34 × 18 mm</text>
<text x="312" y="866" font-size="21" fill="#576675">为暂定壳体包络，不含 USB 插头；不是实测尺寸</text>
<text x="52" y="913" font-size="20" fill="#576675">几何已检查；配合、金属隔离及实物装配尚未验证。配色仅用于区分零件。</text>
</g>
</svg>
'''
    destination = BUILD / "concept-v1-preview.svg"
    destination.write_text(sheet, encoding="utf-8")
    print(destination)


if __name__ == "__main__":
    main()
