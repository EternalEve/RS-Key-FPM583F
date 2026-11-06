<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# V16 薄壁整改打印试装包

针对加工方标出的托架薄壁，增加定位耳、后立柱及上托片的实体截面，并调整配合处。
用户已确认四件均未投产，并允许为强度局部加宽。
详细尺寸、检查范围和剩余薄特征见[制造审查](manufacturing-review-v16.md)。

外形不含外露 USB 为 **51.8 × 32.1 × 19 mm**；前端仍宽 28 mm，尾端圆弧仍按 26 mm。
加宽集中在两颗螺丝附近，没有把全长改成 32.1 mm。
相对 V15-R1，真实 CAD 俯视面积增加约 1.9%。
螺丝中心从 Y=±9.8 移至 ±10.85 mm，X=28.5 mm；底壳、上盖及左右托架须整套使用 V16。
PCB、USB、指纹模块和板件叠层位置沿用原设计，接线通道保留。

## 加工文件

使用 `RS-Key-enclosure-V16-complete-print-fit-prototype.zip`，单位毫米，比例 1:1。

| 文件 | 数量 | 内容 |
| --- | --- | --- |
| `PRINT/v16-base.stl` | 1 | 深底壳、局部加宽的定位耳让位和加宽后托脚插槽 |
| `PRINT/v16-lid.stl` | 1 | 浅上盖、外移后的螺柱及指纹上压环 |
| `PRINT/v16-carrier-left.stl` | 1 | 加厚左托架 |
| `PRINT/v16-carrier-right.stl` | 1 | 加厚右托架 |
| `PRINT/v16-four-parts.3mf` | 1 套 | 同一组四个独立对象的排版模型 |
| `PADS/v16-silicone-pads-1to1.svg` | 1 套 | 四片硅胶垫切割模板，仅切 `cut` 组 |

STL 和 3MF 二选一，总共打印四个塑料件；嘉立创下单逐件上传四个 STL。
STL 已归零，上盖外表面朝下；3MF 不包含打印温度、机器配置或 G-code。
保留普通未增强 PA12、MJF、染黑哑光的要求，四件用同批材料。
**请加工方重新复核红区及本文列出的剩余薄特征，不直接回复接受所有薄壁风险。**

## 五金和硅胶

两颗 M2 × 10 底部螺丝，头部外径约 3.8 mm、高约 2 mm；两颗 M2 嵌件外径 3.6 mm、长 4 mm。
嵌件孔名义直径 3.2 mm、深 5 mm，定位柱/孔仍为 4.8/4.9 mm。
这些紧配合尺寸仍需修配，不能按原始打印精度直接保证。

硅胶沿用 HT-6240、40A、自由厚 0.508 mm、压缩包络 0.42 mm，不含额外胶层。
四片分别为一片外径 20.5 mm、自由内径 18.8 mm 环垫，两片 1.2 × 1.2 mm 尾垫，
以及一条 0.8 × 12 mm 上压垫。SVG 的 10 mm 校准线须先实测。
尾垫槽随立柱移到 Y=±6.5 mm，槽的宽、高、深及垫片本身规格不变。

## 装配

1. 清粉去毛刺，测量定位孔、嵌件孔、托架槽、薄接触唇和 USB 前壁。
2. 断电放置四片硅胶垫，从两侧把托架套住 PCB 裸边；复核焊锡、排针和元件让位。
3. 把 PCB、USB 和托架整体竖直装入底壳，勿掰弯前壁或强压定位耳。
4. 放置指纹模块及环垫，理顺线束，避开螺丝、接缝和压条。
5. 竖直合盖，确认上盖螺柱夹持安装耳、硅胶压在模块台阶而非感应面上。
6. 两颗螺丝从底部装入并均匀锁紧；检查板件翘曲、夹线、金属隔离和模块松动。
7. 实测 USB 轴向阻挡不得超过 1.0 mm。名义 0.8 mm 仍保留 0.2 mm 总偏差预算。

首次试装需检查嵌件、后立柱、定位耳及 USB 两角的裂纹、位移和永久变形。
本包不含载荷额定值、疲劳寿命或实物打印合格结论。

## 检查及复现

正式检查共 111 项，包括 16 个连续材料截面、远处底角保留、圆角芯部及补料、实际 STL 重导入、
PCB/叠层/硅胶/模块/金属/线束净空、限位和连续装入合盖扫掠。
`--without-thickness-fix` 从同一入口恢复旧薄壁尺寸，必须让 16 个截面检查失败。
其余缺失对照覆盖前止挡、USB 圆角、承力圆角和网格清理，故意失败的日志不代表正常件不合格。
`--without-local-root-fix` 恢复全局凸包底角算法，必须在远处底角保留检查上失败。

沿用 V15-R1 的二进制网格流程：0.0001 mm 简化、低于 0.001 mm 的短边合并，
最终实际序列化后的增减材料总量不得超过原网格的百万分之一。
总表面预算 0.0012 mm，并验证单精度、保留 6/5/4 位小数时的闭合、方向、法向及单壳体。
切片器须保持面数且零修复；旧 V15 坏网格和自动删面日志必须被检查拒绝。

V15-R1 与 V16 的实际打印网格变化记录在 `checks/v15-r1-design-comparison.json`。
允许改变螺丝周边壳体及托架尾部，区外材料变化仍以百万分之一作为数值预算。
这是设计范围和截面检查，不把材料增量换算成强度增量。

```sh
nix develop .#enclosure -c python enclosure-design/verify_redesign.py --openscad openscad
for flag in --without-thickness-fix --without-local-root-fix --without-front-stop \
  --without-usb-rounds --without-mesh-cleanup --without-precision-cleanup --without-load-roots; do
  control_status=0
  nix develop .#enclosure -c python enclosure-design/verify_redesign.py --openscad openscad "$flag" || control_status=$?
  [ "$control_status" -eq 1 ] || exit 1
done
nix develop .#enclosure -c python enclosure-design/redesign_preview.py
nix develop .#enclosure -c python enclosure-design/usb_slot_preview.py
nix develop .#enclosure -c python enclosure-design/load_root_preview.py
nix develop .#enclosure -c python enclosure-design/thin_wall_preview.py
HOST_TARGET=x86_64-unknown-linux-gnu nix develop -c ./scripts/check.sh > enclosure-design/build/v16-project-check.log 2>&1
nix develop .#enclosure -c python enclosure-design/package_redesign.py
```

七组缺失对照预期退出码为 1；打包器还会逐项核对失败名称与源码哈希，不能用任意错误代替检出缺陷。
几何和项目门禁均通过后才能打包。嘉立创平台审核、实际打印、公差、强度和寿命需后续验证。
