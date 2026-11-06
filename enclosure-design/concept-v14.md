<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# V14 整体打印试装包

本版打印文件保留作历史对照。当前生成器已切换至 [V15](concept-v15.md)，
其承力补强及装配让位取舍见 [V15 承力点审查](load-review-v15.md)。

本版修复 V13 导出网格的退化问题，出具底壳、上盖、左右托架四件完整打印文件。
**外形仍为 51.8 × 28 × 19 mm，不含外露 USB；沿用 V13 全部名义几何、承力截面与配合尺寸。**
USB 槽底保留两处补料式 R0.4，前壁厚 0.8 mm，USB 最小名义净空 0.25 mm。
状态为可导入切片软件的打印试装稿，实物配合、强度及寿命尚未验证。

## 交给加工方的文件

使用 `RS-Key-enclosure-V14-complete-print-fit-prototype.zip`，单位全部为毫米，比例 1:1。

| 文件 | 数量 | 内容 |
| --- | --- | --- |
| `PRINT/v14-base.stl` | 1 | 深底壳、一体前止挡、USB 槽底圆角、指纹下托环 |
| `PRINT/v14-lid.stl` | 1 | 浅上盖、USB 上沿短桥、螺柱及指纹上压环 |
| `PRINT/v14-carrier-left.stl` | 1 | 左 PCB 内托架 |
| `PRINT/v14-carrier-right.stl` | 1 | 右 PCB 内托架，与左件配套 |
| `PRINT/v14-four-parts.3mf` | 1 套 | 同一组四件的排版模型，含毫米单位及各件名称 |
| `PADS/v14-silicone-pads-1to1.svg` | 1 套 | 四片硅胶垫的 1:1 切割模板，仅切 `cut` 组 |

**STL 和 3MF 是同一套四件的两种交付方式，择一使用，总共只打印四个塑料件。**
STL 已逐件归零到 Z = 0，上盖外表面朝下；左右托架是两个不同零件，各打一件。
3MF 仅含模型及排版，没有打印机、材料温度或 G-code；不能将四件融合成一个实体。
原始装配坐标在 CAD 中保留，检查记录包含打印坐标到装配坐标的变换矩阵。

沿用普通未增强 PA12、MJF、染黑哑光的材料与工艺要求，四件使用同批材料。
加工方须确认 0.8 mm 前壁、细筋、盲孔及清粉能力，保持功能面和小孔尺寸；
不要整体缩放模型或自行削薄承力面。3MF 排版不是对任何特定打印机的工艺设置。

## 五金与硅胶清单

| 项目 | 数量 | 规格 |
| --- | --- | --- |
| 合壳螺丝 | 2 | M2 × 10，从底部装入；头部外径约 3.8 mm、高约 2 mm |
| 螺纹嵌件 | 2 | M2、外径 3.6 mm、长 4 mm，装在上盖盲孔 |
| 指纹环垫 | 1 | 外径 20.5 mm、自由内径 18.8 mm |
| PCB 尾端垫 | 2 | 每片 1.2 × 1.2 mm |
| PCB 上压垫 | 1 | 0.8 × 12 mm |

四片垫均沿用实心硅胶 HT-6240、40A、名义厚 0.508 mm；模型按压缩后约 0.42 mm 预留。
不含额外胶层厚度。SVG 中的 10 mm 校准线应先实测，禁止缩放到页面。
嵌件孔名义直径 3.2 mm、深 5 mm；嵌件安装和孔修配应按加工工艺操作，避免穿孔或开裂。

## 装配顺序

1. 清粉、去毛刺，测量前壁、USB 槽、定位孔、嵌件孔和主要配合面。
   托架定位孔 4.9 mm 配 4.8 mm 柱，单边 0.05 mm 是最终修配目标，不能按原始打印精度保证。
2. 断电状态下，把两片尾垫和一条上压垫放入对应槽位，从左右把托架套住 PCB 裸边。
   检查板边托片与焊锡、器件及排针的实际位置。
3. 将 PCB、USB 和托架整体竖直放入深底壳。USB 从向上开放的前槽落入，勿掰弯前壁。
4. 指纹模块台阶放到下托环上，安装环垫并理顺线束，避开螺丝、压条和接缝。
5. 竖直合盖，确认上盖螺柱夹住安装耳，硅胶压在台阶上，感应面自由外露。
6. 从底部装入两颗 M2 × 10 螺丝，逐步均匀锁紧，检查夹线、金属隔离和 PCB 是否翘曲。
7. 实测 USB 轴向阻挡必须不超过 1.0 mm；名义 0.8 mm 仍只剩 0.2 mm 总偏差预算。
   先检查能否插到底，再安排通电和功能验证。

首次试装重点检查前止挡、USB 两角、托架根部、螺柱和嵌件周围的裂纹及永久变形。
本包不提供额定拔出力或疲劳寿命；网格可打印不替代载荷验证。

## 网格修复与验证

V13 的单精度/低精度导出及极小退化面会导致本机 OpenSCAD 再导入失败。
V14 保持名义 CAD 程序不变，先导出二进制原网格，再用 Manifold 在 **0.00001 mm**
表面容差内整理三角面，按 17 位有效数字写出 STL。不是加厚或削薄 CAD 壁厚。
整理结果须保留原顶点子集，包围盒变化不超过该容差，材料差集比例不超过百万分之一。
每件另查零面积面、闭合、有向边一致性、正体积及单连通实体。

完整机械检查共 59 项：保留 V13 的几何、限位、配合、USB 净空与连续装配扫掠检查，
增加四件实际打印 STL 重导入后的单实体布尔检查。打包时还用 PrusaSlicer 逐件导入 STL
及四件 3MF，并重新核对源文件、CAD、STL、ZIP 的哈希及完整性。
原始 CAD 与 V13 去除注释后的程序逐字相同，证明名义承力截面及配合尺寸没有改变。

```sh
nix develop .#enclosure -c python enclosure-design/verify_redesign.py --openscad openscad
nix develop .#enclosure -c python enclosure-design/redesign_preview.py
nix develop .#enclosure -c python enclosure-design/usb_slot_preview.py
HOST_TARGET=x86_64-unknown-linux-gnu nix develop -c ./scripts/check.sh > enclosure-design/build/v14-project-check.log 2>&1
nix develop .#enclosure -c python enclosure-design/package_redesign.py
```

外壳专用环境从项目现有 `flake.lock` 取 OpenSCAD、Manifold、Trimesh 和 PrusaSlicer，
只用于模型整理与独立检查，不加入固件依赖，也不改工具链锁定版本。
机械检查独立于默认项目 CI；完整项目门禁与机械检查都须通过才能打包。

| 正式入口的缺失对照 | 预期结果 |
| --- | --- |
| `--without-front-stop` | 前止挡拦截检查失败，移除止挡的放行对照仍通过，退出 1 |
| `--without-usb-rounds` | 左右两角的补料检查都失败，其余前置检查通过，退出 1 |
| `--without-mesh-cleanup` | 底壳 STL 重导入失败，另外三件通过，退出 1 |

缺失整理的对照必须在实际重导入行失败，不能用程序崩溃、文件丢失或其他断言代替。
`checks/` 内保留这些故意失败的对照记录，正常总结果以 `geometry-and-meshes.json` 为准。
