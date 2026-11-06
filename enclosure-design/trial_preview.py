#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Render the exported V11 parts for review without OpenGL or altering CAD meshes."""

import math
import struct

from PIL import Image, ImageDraw, ImageFont

from preview import dot, read_off, shade
from reference_preview import FONT
from trial_model import ROOT, parameters


BUILD = ROOT / "build"
COLORS = {"base": (76, 90, 107), "lid": (143, 156, 172),
          "carrier-left": (34, 155, 169), "carrier-right": (34, 155, 169),
          "pads-installed": (211, 139, 68), "sensor": (206, 213, 216), "usb": (175, 184, 194)}


def rasterize(width, height, triangles):
    depth = [-math.inf]*(width*height)
    pixels = bytearray(b"\xff\xff\xff"*(width*height))
    for points, rgb in triangles:
        a, b, c = points
        area = (b[1]-c[1])*(a[0]-c[0]) + (c[0]-b[0])*(a[1]-c[1])
        if abs(area) < 1e-8:
            continue
        x0 = max(0, math.ceil(min(v[0] for v in points)-0.5))
        x1 = min(width-1, math.floor(max(v[0] for v in points)-0.5))
        y0 = max(0, math.ceil(min(v[1] for v in points)-0.5))
        y1 = min(height-1, math.floor(max(v[1] for v in points)-0.5))
        du, dv = (b[1]-c[1])/area, (c[1]-a[1])/area
        color = bytes.fromhex(rgb.removeprefix("#"))
        for y in range(y0, y1+1):
            u = ((b[1]-c[1])*(x0+0.5-c[0]) +
                 (c[0]-b[0])*(y+0.5-c[1]))/area
            v = ((c[1]-a[1])*(x0+0.5-c[0]) +
                 (a[0]-c[0])*(y+0.5-c[1]))/area
            for x in range(x0, x1+1):
                if u >= -1e-8 and v >= -1e-8 and u+v <= 1+1e-8:
                    z = u*a[2] + v*b[2] + (1-u-v)*c[2]
                    index = y*width+x
                    if z > depth[index]:
                        depth[index] = z
                        pixels[index*3:index*3+3] = color
                u += du
                v += dv
    return Image.frombytes("RGB", (width, height), bytes(pixels))


def main():
    p = parameters()
    meshes = {name: read_off(BUILD / f"v11-{name}.off")[1] for name in COLORS}
    board_data = (BUILD / "reference/board.stl").read_bytes()
    count = struct.unpack_from("<I", board_data, 80)[0]
    if len(board_data) != 84+50*count:
        raise ValueError("Expected binary official reference STL")
    board = []
    for index in range(count):
        values = struct.unpack_from("<12fH", board_data, 84+50*index)
        board.append([(p["pcb_x"]+p["pcb_length"]-values[i+1],
                       values[i]-p["pcb_width"]/2, p["board_top_z"]+values[i+2])
                      for i in (3, 6, 9)])
    image = Image.new("RGB", (1800, 1240), "#edf2f6")
    draw = ImageDraw.Draw(image)

    def text(x, y, content, size=25, fill="#26394c"):
        draw.text((x, y), content, fill=fill, font=ImageFont.truetype(FONT, size))

    def panel(box, exploded=False, inside=False, front=False, keys=False, carrier=None):
        projection = (((0.5812,-0.8137,0),(0.4069,0.2906,0.866),(-0.7047,-0.5034,0.5)) if carrier else
                      ((0.4, 0.9165, 0), (-0.4583, 0.2, 0.866), (0.7937, -0.3464, 0.5)) if front or keys else
                      ((1, 0, 0), (0, 1, 0), (0, 0, 1)) if inside else
                      ((0.954, -0.298, 0), (0.164, 0.525, 0.836), (-0.249, -0.798, 0.55)))
        right, up, depth = projection
        scene = []
        scene_meshes = {"lid": read_off(BUILD / "v11-lid-front-detail.off")[1]} if front else meshes
        if keys:
            scene_meshes = {name: read_off(BUILD / f"v11-front-key-{name}.off")[1]
                            for name in ("base", "lid")}
        if carrier:
            name = "carrier-v10-left" if carrier == "v10" else "carrier-left"
            scene_meshes = {"carrier-left": read_off(BUILD / f"v11-{name}.off")[1]}
        for name, faces in scene_meshes.items():
            if inside and name in ("lid", "sensor", "usb"):
                continue
            lift = (24 if name == "lid" else 12 if name in ("sensor", "usb") else
                    8 if name not in ("base", "lid") else 0) if exploded else 0
            if keys and name == "lid":
                lift = 9
            scene.extend(([(v[0], v[1], v[2]+lift) for v in face], COLORS[name])
                         for face in faces)
        if not inside and not front and not keys and not carrier:
            for face in board:
                lift = 12 if exploded else 0
                rgb = (54, 109, 145) if sum(v[2] for v in face)/3 < 13.51 else (95, 107, 120)
                scene.append(([(v[0], v[1], v[2]+lift) for v in face], rgb))
            center = (p["sensor_x"], 0, p["case_height"] + (12 if exploded else 0) + 0.01)
            rim = [(center[0]+8.5*math.cos(i*math.pi/32),
                    8.5*math.sin(i*math.pi/32), center[2]) for i in range(64)]
            scene.extend(([center, rim[i], rim[(i+1)%64]], (25, 33, 40)) for i in range(64))
        vertices = [v for face, _ in scene for v in face]
        xs, ys = [dot(v, right) for v in vertices], [-dot(v, up) for v in vertices]
        low_x, high_x, low_y, high_y = min(xs), max(xs), min(ys), max(ys)
        x, y, width, height = box
        scale = min(width/(high_x-low_x), height/(high_y-low_y))
        ox, oy = x+(width-(high_x-low_x)*scale)/2, y+(height-(high_y-low_y)*scale)/2
        triangles = []
        for face, rgb in scene:
            polygon = [(ox-x+(dot(v, right)-low_x)*scale,
                        oy-y+(-dot(v, up)-low_y)*scale, dot(v, depth))
                       for v in face]
            for index in range(1, len(face)-1):
                triangles.append(([polygon[0], polygon[index], polygon[index+1]], shade(face, rgb)))
        image.paste(rasterize(int(width), int(height), triangles), (int(x), int(y)))

    text(48, 30, "RS-Key / V11 加固内托架试装壳", 40)
    text(48, 100, "51.8 × 33.6 × 19 mm · 两组竖插 T 形脚 · 0.8 mm 上盖一体前壁", 27, "#607487")
    text(1330, 60, "试装版 / 尚未实物验证", 27, "#a35e1a")
    for box in ((40, 160, 880, 790), (920, 160, 1760, 790), (40, 820, 1080, 1160)):
        draw.rounded_rectangle(box, radius=20, fill="white")
    text(70, 181, "01 / 合壳", 28)
    text(950, 181, "02 / 分解", 28)
    panel((82, 265, 756, 420))
    panel((964, 248, 746, 460), exploded=True)
    text(70, 714, "板外托片及支柱加固；侧梁补宽、补高", 23, "#a35e1a")
    text(950, 714, "官方主板投影；未重建实物背叠板及线束", 23, "#607487")
    text(70, 838, "03 / 去盖、隐藏电路：内部固定结构", 27)
    panel((90, 902, 920, 225), inside=True)
    text(90, 967, "← USB 方向", 22, "#607487")
    text(1120, 845, "青色：从两侧装入的内托架", 25, "#258594")
    text(1120, 893, "橙色：受压硅胶垫空间示意", 25, "#ab681f")
    text(1120, 953, "USB 名义阻挡 0.8 mm", 26)
    text(1120, 999, "实物验收必须 ≤ 1.0 mm", 26, "#a35e1a")
    text(1120, 1061, "材质：普通 PA12 / MJF", 25)
    text(48, 1180, "配色仅区分结构；实际可统一黑色。模块及托架配合、USB 插入和金属隔离仍须断电试装核对。", 23, "#607487")
    destination = BUILD / "concept-v11-review.png"
    image.save(destination)
    print(destination)
    image = Image.new("RGB", (1800, 1060), "#edf2f6")
    draw = ImageDraw.Draw(image)
    text(48, 30, "V11 / 前壁与加强筋根部圆角 · 从壳内侧观察", 39)
    text(48, 100, "直接使用导出网格；剖去顶板，显示前端承力根部", 26, "#607487")
    draw.rounded_rectangle((40,160,1050,980),radius=20,fill="white")
    panel((75,200,940,730),front=True)
    text(1100, 210, "前壁名义厚度 0.8 mm", 32)
    text(1100, 280, "前壁—顶板：R0.6 内圆角", 27)
    text(1100, 360, "外侧筋：根部 R0.8 / 向后 6 mm", 26)
    text(1100, 410, "小筋：侧根 R0.4 / 向后 4 mm", 26)
    text(1100, 470, "恢复 V7 筋芯，圆角向外补料", 26, "#258594")
    text(1100, 510, "小筋 Z13.7 / 大筋 Z12.1", 25, "#258594")
    text(1100, 555, "小筋下缘保留平段及板面净空", 25)
    text(1100, 600, "合壳后 USB 孔：9.2 × 3.5 mm", 25)
    text(1100, 650, "上端经筋与顶板传力", 27)
    text(1100, 700, "下端插脚接触挡肩后向底壳传力", 25)
    text(1100, 810, "几何加强设计 / 尚未验证拉力", 25, "#a35e1a")
    text(48, 1000, "普通 PA12 / MJF；前端阻挡实测须 ≤ 1 mm。打样后须验证实际拔出力、变形和反复插拔。", 25, "#607487")
    destination = BUILD / "concept-v11-front-detail.png"
    image.save(destination)
    print(destination)
    image = Image.new("RGB", (1800, 1120), "#edf2f6")
    draw = ImageDraw.Draw(image)
    text(48, 30, "V11 / 前壁下端 T 形插脚与底壳挡肩", 39)
    text(48, 100, "真实网格局部裁切；上盖上移 9 mm；另一侧镜像对称", 26, "#607487")
    draw.rounded_rectangle((40,160,1050,1030),radius=20,fill="white")
    panel((75,195,940,790),keys=True)
    text(1100, 200, "上盖竖直插入；螺丝保持合盖", 28)
    text(1100, 275, "T 形头：2.6 × 2.0 mm", 27)
    text(1100, 325, "插脚高度：6 mm / 颈宽 1.4 mm", 25)
    text(1100, 400, "前挡肩沿 X 厚 1.6 mm", 27)
    text(1100, 450, "挡肩连接底板；不是薄弹性卡钩", 25)
    text(1100, 525, "向外目标间隙 0.1 mm", 28, "#258594")
    text(1100, 575, "横向名义间隙每侧 0.2 mm", 25)
    text(1100, 625, "须试配修整，不保证打印即合格", 25, "#a35e1a")
    text(1100, 710, "接触挡肩后，下端向底壳传力", 26)
    text(1100, 760, "配合有间隙，不能视为完全固支", 25)
    text(1100, 865, "接触/装配几何检查；尚未验证拉力", 24, "#a35e1a")
    text(48, 1060, "插脚在 PCB 两侧；前端加宽至 33.6 mm，让出完整挡肩和槽壁。USB 名义阻挡仍为 0.8 mm。", 24, "#607487")
    destination = BUILD / "concept-v11-key-detail.png"
    image.save(destination)
    print(destination)
    image = Image.new("RGB", (1800, 1040), "#edf2f6")
    draw = ImageDraw.Draw(image)
    text(48, 30, "V11 / 左内托架加固前后对比", 40)
    text(48, 100, "相同视角的实际导出网格；右托架采用相同局部加固", 26, "#607487")
    for box in ((40,165,880,755),(920,165,1760,755)):
        draw.rounded_rectangle(box,radius=20,fill="white")
    text(70, 190, "V10 / 原托架", 31)
    text(950, 190, "V11 / 保留原芯，向板外补料", 31)
    panel((70,270,780,430),carrier="v10")
    panel((950,270,780,430),carrier="v11")
    text(70, 795, "前托根部：板外区域 1 → 2 mm；厚薄交接 R0.4", 28)
    text(70, 850, "前支柱横向 1.2 → 2 mm；后段侧梁目标宽 2 mm、高 6.5 mm", 27)
    text(70, 905, "前段上缘跨过挡肩，连接安装耳；板内托片保持 1 mm 以避让针脚", 26)
    text(70, 975, "外壳尺寸及 USB 阻挡不变；几何补强，未验证破坏载荷或寿命", 25, "#a35e1a")
    destination = BUILD / "concept-v11-carrier-detail.png"
    image.save(destination)
    print(destination)


if __name__ == "__main__":
    main()
