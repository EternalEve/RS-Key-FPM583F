#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Compare true plan envelopes and render exported V16 candidate meshes."""

import json

from PIL import Image, ImageDraw, ImageFont

from preview import dot, read_off, shade
from redesign_model import ROOT
from reference_preview import FONT
from trial_preview import COLORS, rasterize
from verify_trial import volume


BUILD = ROOT / "build"


def outline(path):
    _, faces = read_off(path)
    adjacent = {}
    for face in faces:
        if max(v[2] for v in face)-min(v[2] for v in face) < 0.5:
            continue
        for a,b in zip(face,face[1:]+face[:1]):
            if abs(a[2]) < 1e-6 and abs(b[2]) < 1e-6:
                a,b = tuple(a[:2]),tuple(b[:2])
                adjacent.setdefault(a,set()).add(b)
                adjacent.setdefault(b,set()).add(a)
    if not adjacent or any(len(neighbors) != 2 for neighbors in adjacent.values()):
        raise ValueError("Footprint boundary is not a closed polygon")
    first = min(adjacent)
    boundary, previous, current = [first], None, first
    while True:
        next_point = next(p for p in sorted(adjacent[current]) if p != previous)
        if next_point == first:
            break
        boundary.append(next_point)
        previous,current = current,next_point
        if len(boundary) > len(adjacent):
            raise ValueError("Footprint boundary failed to close")
    if len(boundary) != len(adjacent):
        raise ValueError("Footprint contains more than one boundary")
    return boundary


def area(points):
    return abs(sum(a[0]*b[1]-b[0]*a[1] for a,b in zip(points,points[1:]+points[:1])))/2


def main():
    report = json.loads((BUILD/"v16-geometry-checks.json").read_text())
    if not report["all_passed"]:
        raise RuntimeError("Candidate geometry must pass before rendering the review sheet")
    names = ("base","lid","carrier-left","carrier-right","sensor","usb","pads-installed","board")
    meshes = {name:read_off(BUILD/f"v16-{name}.off")[1] for name in names}
    image = Image.new("RGB",(1800,1440),"#edf2f6")
    draw = ImageDraw.Draw(image)

    def text(x,y,label,size=25,color="#26394c"):
        draw.text((x,y),label,fill=color,font=ImageFont.truetype(FONT,size))

    def panel(box,exploded=False,inside=False):
        right,up,depth = ((1,0,0),(0,1,0),(0,0,1)) if inside else (
            (0.954,-0.298,0),(0.164,0.525,0.836),(-0.249,-0.798,0.55))
        scene = []
        for name,faces in meshes.items():
            if inside and name in ("lid","sensor","board","usb","pads-installed"):
                continue
            lift = (24 if name=="lid" else 13 if name in ("board","sensor","usb") else
                    8 if name in ("carrier-left","carrier-right","pads-installed") else 0) if exploded else 0
            color = (48,111,149) if name=="board" else COLORS[name]
            scene.extend(([(v[0],v[1],v[2]+lift) for v in face],color) for face in faces)
        vertices = [v for face,_ in scene for v in face]
        xs,ys = [dot(v,right) for v in vertices],[-dot(v,up) for v in vertices]
        low_x,high_x,low_y,high_y = min(xs),max(xs),min(ys),max(ys)
        x,y,width,height = box
        scale = min(width/(high_x-low_x),height/(high_y-low_y))
        ox,oy = (width-(high_x-low_x)*scale)/2,(height-(high_y-low_y)*scale)/2
        triangles = []
        for face,color in scene:
            polygon = [(ox+(dot(v,right)-low_x)*scale,oy+(-dot(v,up)-low_y)*scale,dot(v,depth)) for v in face]
            for i in range(1,len(face)-1):
                triangles.append(([polygon[0],polygon[i],polygon[i+1]],shade(face,color)))
        image.paste(rasterize(width,height,triangles),(x,y))

    old = outline(BUILD/"v15-r1-footprint.off")
    new = outline(BUILD/"v16-footprint.off")
    metrics = {"v15_r1_footprint_mm2":area(old),"v16_footprint_mm2":area(new),
               "maximum_width_increase_percent":100*(32.1-28)/28,
               "footprint_increase_percent":100*(area(new)-area(old))/area(old),
               "v15_r1_plastic_mm3":sum(volume(BUILD/f"v15-r1-{n}.off")
                                      for n in names[:4]),
               "v16_plastic_mm3":sum(report["meshes"][n]["volume_mm3"] for n in names[:4])}
    (BUILD/"v16-comparison.json").write_text(json.dumps(metrics,indent=2)+"\n")
    text(48,28,"RS-Key / V16 薄壁整改",40)
    text(48,92,"深底壳直接承受 PCB 前止挡载荷 · 浅上盖压紧 · 保留可拆左右托架",27,"#607487")
    text(1310,42,"候选稿 / 强度待验证",27,"#a35e1a")
    for box in ((40,160,1760,570),(40,600,880,1080),(920,600,1760,1080),(40,1110,1760,1370)):
        draw.rounded_rectangle(box,radius=20,fill="white")
    text(70,180,"01 / 同比例俯视轮廓",28)
    scale = 8.7
    for points,origin,color,label in ((old,(150,380),"#c8d0d9","V15-R1 · 51.8 × 28 × 19 mm"),
                                      (new,(1030,380),"#68aeb8","V16 · 51.8 × 32.1 × 19 mm")):
        draw.polygon([(origin[0]+x*scale,origin[1]+y*scale) for x,y in points],fill=color)
        text(origin[0],530,label,26)
    text(70,610,"02 / V16 合壳",28)
    text(950,610,"03 / 分解与装入方向",28)
    panel((85,680,735,310))
    panel((965,665,730,350),exploded=True)
    text(70,1025,"USB 孔上沿随上盖；前止挡与底壳为一体",22,"#607487")
    text(950,1025,"图中板件为官方器件包络，不包含未知手工线束",22,"#607487")
    text(70,1125,"04 / 去盖：四件配套更新，定位耳与后立柱加厚",27)
    panel((85,1190,690,145),inside=True)
    text(850,1182,"局部最大宽度增加 4.1 mm；前端仍宽 28 mm",27,"#258594")
    text(850,1232,f'实际俯视面积增加 {metrics["footprint_increase_percent"]:.1f}%；高度及长度不变',25)
    text(850,1280,"0.8 mm 前壁仍承力；小间隙仍需修配",25,"#a35e1a")
    text(48,1390,"真实导出网格与轮廓；颜色仅区分零件。几何通过不代表承载能力、寿命或制造公差已通过。",23,"#607487")
    destination = BUILD/"concept-v16-review.png"
    image.save(destination)
    print(destination)
    print(json.dumps(metrics,indent=2))


if __name__ == "__main__":
    main()
