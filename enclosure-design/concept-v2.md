<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# 紧凑外壳初稿 V2

状态：在 V1 基础上调整布局的讨论稿。未核对实物尺寸，不用于加工下单。

后续用户将板件总厚度上限修正为约 14 mm，本版 18 mm 壳高不满足当前壁厚与净空预算。
长宽布局继续作为参考，当前高度预算见 [V3](concept-v3.md)。

## 缩小方式

壳体暂定包络由 V1 的 64 × 34 × 18 mm 调整为 58 × 26 × 18 mm，
均不包含 USB 插头。宽度减少约 24%，矩形包络体积减少约 31%；
这不是打印材料用量的变化，也不是实测装配尺寸。

保持现有两块 PCB 叠放、模块与 PCB 分开布置，以及四颗底部螺丝。
前一对螺柱移至 PCB 前端、USB 两侧；后一对移至 PCB 后端与圆形模块之间的两侧空隙。
这样无需沿 PCB 全长为两侧螺柱额外增加壳宽，也无需在指纹模块末端增加螺丝尾舱。

模型中 PCB 后缘到模块圆柱包络前缘留 3.5 mm，线束从下方通过。
真实连接器、出线弯曲、焊脚及元件外伸尚未建模，可能要求增大该间距。
壳高仍保持 18 mm，待实测叠层和焊脚后再判断是否能降低。

## 绝缘约束

沿用 V1 的嵌件盲孔、塑料螺柱、绝缘隔壁及不穿 PCB 焊孔的要求。
金属件规格和允许螺丝长度仍须按商家实际嵌件、底部焊脚及加工公差确定。

模型中的名义约束包括：

- 螺柱外表面与 PCB 矩形包络之间大于 0.6 mm 的间隙。
- 嵌件孔与指纹开口之间大于 1.2 mm 的几何距离。
- 前端嵌件孔与 USB 开口之间大于 1.2 mm 的几何距离。
- 嵌件孔周围保留至少 1.2 mm 的螺柱塑料厚度，孔顶仍封闭。

这些值是初稿几何筛查条件，不是通用电气间隙标准，也不证明防短路合格。
模型没有涵盖实际金属件、焊点和绝缘层的完整边界；实物核对仍是定稿条件。

## 模型与预览

```sh
nix develop -c python enclosure-design/model.py --variant v2
```

CAD 源码生成到 `build/concept-v2.scad`。视图选项与 V1 相同。
在另行提供 OpenSCAD 与 librsvg 的开发环境内，生成等比例对比图：

```sh
nix develop -c openscad -D 'part="assembled"' -o enclosure-design/build/v2-assembled-check.off enclosure-design/build/concept-v2.scad
nix develop -c python enclosure-design/preview.py --comparison
nix develop -c rsvg-convert -o enclosure-design/build/concept-comparison.png enclosure-design/build/concept-comparison.svg
```

对比图还需要 V1 的 `build/assembled-check.off`，生成方法见 [V1 说明](concept-v1.md)。
V1 保留作为对照；两版均缺少定稿的模块压环、PCB 压条、定位止口和按钮机构。

V2 底壳、上盖及两版装配网格已通过 OpenSCAD 实体几何检查，
脚本语法、预览 SVG 结构及 SPDX 检查通过。完整软件门禁未运行；
上述检查不替代配合、线束、实际金属边界和实物装配的核对。

## 定稿前

优先核对两块 PCB 的实际长宽、叠层总高及 USB 两侧空隙。
再核对指纹模块背部连接器与线束能否放入过渡区，以及螺柱位置是否与焊点避让。
尺寸不符时增大对应区域，不通过削薄绝缘结构或挤压元件来维持当前包络。
