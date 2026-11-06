#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Show the V16 blends on this revision's exported unblended cores."""

import json

from PIL import Image, ImageDraw, ImageFont

from preview import dot, read_off, shade
from redesign_model import ROOT
from reference_preview import FONT
from trial_preview import rasterize


BUILD = ROOT/"build"


def clip_below(face, height):
    clipped = []
    for a,b in zip(face,face[1:]+face[:1]):
        if a[2] <= height:
            clipped.append(a)
        if (a[2] <= height) != (b[2] <= height):
            fraction = (height-a[2])/(b[2]-a[2])
            clipped.append(tuple(a[i]+fraction*(b[i]-a[i]) for i in range(3)))
    return clipped


def main():
    report = json.loads((BUILD/"v16-geometry-checks.json").read_text())
    if not report["all_passed"]:
        raise ValueError("V16 geometry checks must pass before rendering")
    image = Image.new("RGB",(1800,1330),"#edf2f6")
    draw = ImageDraw.Draw(image)

    def text(x,y,label,size=25,color="#26394c"):
        draw.text((x,y),label,fill=color,font=ImageFont.truetype(FONT,size))

    def panel(part,box,flip=False,back=False,cut=None):
        right,up,depth = ((0.894,0.447,0),(-0.2,0.4,0.894),(0.4,-0.8,0.447)) if back else (
            (0.894,-0.447,0),(0.436,0.872,0.224),(-0.1,-0.2,0.975))
        scene = []
        for path,color in ((BUILD/f"v16-core-{part}.off",(144,157,170)),
                           (BUILD/f"v16-load-roots-add-to-{part}.off",(227,133,43))):
            faces = read_off(path)[1]
            if cut is not None:
                faces = [clipped for face in faces if len(clipped := clip_below(face,cut)) >= 3]
            scene.extend(([(v[0],v[1],19-v[2] if flip else v[2]) for v in face],color)
                         for face in faces)
        points = [v for face,_ in scene for v in face]
        xs,ys = [dot(v,right) for v in points],[-dot(v,up) for v in points]
        low_x,high_x,low_y,high_y = min(xs),max(xs),min(ys),max(ys)
        x,y,width,height = box
        scale = min(width/(high_x-low_x),height/(high_y-low_y))
        ox,oy = (width-(high_x-low_x)*scale)/2,(height-(high_y-low_y)*scale)/2
        triangles = []
        for face,color in scene:
            polygon = [(ox+(dot(v,right)-low_x)*scale,oy+(-dot(v,up)-low_y)*scale,
                        dot(v,depth)) for v in face]
            triangles.extend(([polygon[0],polygon[i],polygon[i+1]],shade(face,color))
                             for i in range(1,len(face)-1))
        image.paste(rasterize(width,height,triangles),(x,y))

    text(45,25,"RS-Key / V16 承力连接补强",40)
    text(45,88,"灰色：V16 未加圆角芯部   橙色：真实 CAD 圆角差集   外形：51.8 × 32.1 × 19 mm",26)
    for box in ((35,145,890,710),(915,145,1765,710),(35,735,890,1235),(915,735,1765,1235)):
        draw.rounded_rectangle(box,radius=20,fill="white")
    text(65,165,"01 / 底壳剖视：隐藏 Z > 13.8 mm",28)
    panel("base",(65,225,790,380),cut=13.8)
    text(65,628,"前角整高 R0.5；支腿底脚 R0.6、环下 R0.5",24)
    text(65,669,"支腿原芯外补 0.1 mm 圆滑包络；座面及通孔保留",23,"#607487")
    text(945,165,"02 / 上盖内侧：承力筋根",28)
    panel("lid",(945,225,790,380),flip=True)
    text(945,628,"斜承力筋及两条纵筋与顶板间补 R0.4",24)
    text(945,669,"螺柱已有 R0.8；孔径和嵌件位置保留",23,"#607487")
    text(65,755,"03 / 左托架剖视：隐藏 Z > 8.2 mm",28)
    panel("carrier-left",(65,815,790,285),back=True,cut=8.2)
    text(65,1130,"底脚上方、安装耳下方补 R0.4；右件对称处理",24)
    text(65,1175,"保留 PCB 接触面、垫槽及安装耳夹持面",23,"#607487")
    text(945,755,"04 / 验证与边界",28)
    for y,label in ((830,"本版未加圆角芯部：无正体积损失"),
                    (890,"19 处具体承力连接：探测到新增材料"),
                    (950,"前角另查 0.2 mm 装入预留及放大对照"),
                    (1010,"装入、合盖、USB、指纹及金属隔离复核"),
                    (1100,"未做实物载荷、疲劳或有限元验证")):
        text(945,y,label,25,"#a35e1a" if y==1100 else "#26394c")
    text(45,1275,"按实际导出网格绘制；新增材料的颜色不表示应力、变形或强度改善百分比。",25,"#607487")
    destination = BUILD/"concept-v16-load-roots.png"
    image.save(destination)
    print(destination)


if __name__ == "__main__":
    main()
