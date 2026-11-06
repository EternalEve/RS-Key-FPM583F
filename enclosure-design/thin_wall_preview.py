#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Show nominal wall sections and the actual non-convex case footprint."""

import json
from PIL import Image, ImageDraw, ImageFont

from redesign_model import ROOT, parameters
from redesign_preview import outline, area
from reference_preview import FONT


BUILD = ROOT/"build"


def main():
    report = json.loads((BUILD/"v16-geometry-checks.json").read_text())
    if not report["all_passed"]:
        raise ValueError("Geometry checks must pass before rendering")
    p = parameters()
    image = Image.new("RGB",(1800,1320),"#edf2f6")
    draw = ImageDraw.Draw(image)

    def text(x,y,label,size=26,color="#26394c"):
        draw.text((x,y),label,fill=color,font=ImageFont.truetype(FONT,size))

    text(45,25,"RS-Key / V16 加工薄壁整改",40)
    text(45,87,"名义剖面尺寸；非应力仿真。四件配套更新，PCB / USB / 指纹位置保留。",26)
    for box in ((35,145,880,680),(920,145,1765,680),(35,710,1765,1250)):
        draw.rounded_rectangle(box,radius=20,fill="white")
    text(65,168,"01 / 后立柱硅胶槽：两侧剩料",29)
    scale = 90
    for x,w,depth,label,wall,color in ((110,2.4,1.6,"V15-R1",0.4,"#bf715d"),(475,4,1.65,"V16",1.2,"#369eab")):
        y=290
        draw.rectangle((x,y,x+w*scale,y+depth*scale),fill=color)
        left=x+(w-1.6)/2*scale
        draw.rectangle((left,y,left+1.6*scale,y+0.37*scale),fill="white")
        text(x,225,label,28)
        text(x,490,f"两侧各 {wall:.1f} mm",27)
        text(x,535,f"柱宽 {w:.1f} / 槽宽 1.6",24,"#607487")
    text(65,615,"内侧边界保留；新增宽度向外补，线束通道仍宽 8.8 mm。",23)
    text(950,168,"02 / 定位耳：孔径仍为 4.9 mm",29)
    for x,d,label,color in ((1110,6.4,"V15-R1", "#bf715d"),(1505,p["carrier_ear_diameter"],"V16", "#369eab")):
        y=390; radius=d*20
        draw.ellipse((x-radius,y-radius,x+radius,y+radius),fill=color)
        r=4.9*20
        draw.ellipse((x-r,y-r,x+r,y+r),fill="white")
        text(x-115,225,label,28)
        text(x-150,570,f"环壁 {(d-4.9)/2:.2f} mm",28)
    text(950,625,"上托片槽后：0.78 → 1.38 mm；上盖导向筋：0.8 → 1.2 mm",22)
    text(65,735,"03 / 实际俯视轮廓：局部扩展，长度和高度保留",29)
    old,new = outline(BUILD/"v15-r1-footprint.off"),outline(BUILD/"v16-footprint.off")
    for shape,color in ((new,"#66aeb8"),(old,"#c9d2dc")):
        draw.polygon([(190+x*12,995-y*12) for x,y in shape],fill=color)
    text(1160,840,"灰：V15-R1   青：V16 新外形",26)
    text(1160,900,"前端宽 28 mm",28)
    text(1160,955,"局部最宽 32.1 mm",28)
    text(1160,1010,"长度 51.8 / 高度 19 mm",28)
    text(1160,1075,f"俯视面积增加 {(area(new)/area(old)-1)*100:.1f}%",28,"#258594")
    text(65,1200,"USB 前壁仍为 0.8 mm，须单独交加工方复审；不宣称整机全部壁厚已超过 1 mm。",25,"#a35e1a")
    text(45,1275,"名义尺寸及 CAD 轮廓；尚无实物承载、疲劳、打印公差或加工方放行结论。",24,"#607487")
    destination = BUILD/"concept-v16-thin-walls.png"
    image.save(destination)
    print(destination)


if __name__ == "__main__":
    main()
