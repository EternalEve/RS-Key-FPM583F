<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# V15-R1 网格修复打印试装包

加工方又发现了名义几何中的托架薄壁，现由 [V16](concept-v16.md) 整套替换，不能混用孔位。
本页保留网格修复历史；薄壁整改见 [V16 制造审查](manufacturing-review-v16.md)。

本版修复 V15 的微小三角面在精度转换后塌陷的问题，替换加工网格；
详见[网格修复记录](mesh-repair-v15-r1.md)。V15 名义 CAD、参数和承力结构逐项保持一致，
承力连接的设计依据仍见 [V15 承力点审查](load-review-v15.md)。
**外形仍为 51.8 × 28 × 19 mm，不含外露 USB；PCB、模块、螺丝位置及既有配合尺寸保留。**
指纹支腿底脚 R0.6、托环连接 R0.5，上盖筋根与尾止挡连接 R0.4。
三根指纹支腿在原柱芯外围增加 0.1 mm 圆滑包络，内侧保留原模块通孔。
图中前内角沿整条竖角补 R0.5，并单独检查托架竖直装入时的 0.2 mm 预留。
USB 槽底保留 R0.4、前壁厚 0.8 mm、USB 最小名义净空 0.25 mm。
状态为可导入切片软件的打印试装稿，实物配合、强度及寿命尚未验证。

## 交给加工方的文件

使用 `RS-Key-enclosure-V15-R1-complete-print-fit-prototype.zip`，单位全部为毫米，比例 1:1。

| 文件 | 数量 | 内容 |
| --- | --- | --- |
| `PRINT/v15-r1-base.stl` | 1 | 深底壳、一体前止挡、USB 槽底圆角、指纹下托环 |
| `PRINT/v15-r1-lid.stl` | 1 | 浅上盖、USB 上沿短桥、螺柱及指纹上压环 |
| `PRINT/v15-r1-carrier-left.stl` | 1 | 左 PCB 内托架 |
| `PRINT/v15-r1-carrier-right.stl` | 1 | 右 PCB 内托架，与左件配套 |
| `PRINT/v15-r1-four-parts.3mf` | 1 套 | 同一组四件的排版模型，含毫米单位及各件名称 |
| `PADS/v15-r1-silicone-pads-1to1.svg` | 1 套 | 四片硅胶垫的 1:1 切割模板，仅切 `cut` 组 |

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

## 几何、网格与装配验证

原网格先按 **0.0001 mm** 容差整理，再合并长度低于 **0.001 mm** 的短边，
保留已有顶点位置，拒绝合并位移超过预算的顶点簇；去除短边合并后塌陷的面。
随后按二进制 STL 的单精度坐标写出，按实际保存的坐标重新计算法向量。
总表面误差预算为 **0.0012 mm**，包围盒及实际序列化后的材料差集都须复核，
材料差异比例仍不得超过百万分之一；没有全局缩放、重塑承力结构或改变配合尺寸。

最终文件不经自动修复，逐件检查零面积面、每条边两面共用及方向相反、
单一闭合正体积实体、存储法向量，并验证单精度及保留 6、5、4 位小数时仍成立。

完整检查共 90 项，包括四件原 CAD 芯部无材料损失、19 处补强材料探测、
前角 0.2 mm 装入预留及 R2.0 放大干涉对照，以及原有的限位、配合、USB 净空、连续装配和 STL 重导入检查。
打包另以 V14 和 V15 实际打印网格作材料对照，并用 PrusaSlicer 导入四件 STL 和 3MF；
必须保持面数一致，任何非零修复计数都会拒绝打包。旧 V15 四件须被精度检查拒绝，
旧底壳的删去 20 面日志须被切片导入检查拒绝。核对源文件、CAD、STL、ZIP 的哈希及完整性。
嘉立创平台本身仍需重新上传新文件确认，本地检查通过不代表已获得平台审核结论。

```sh
nix develop .#enclosure -c python enclosure-design/verify_redesign.py --openscad openscad
nix develop .#enclosure -c python enclosure-design/redesign_preview.py
nix develop .#enclosure -c python enclosure-design/usb_slot_preview.py
nix develop .#enclosure -c python enclosure-design/load_root_preview.py
HOST_TARGET=x86_64-unknown-linux-gnu nix develop -c ./scripts/check.sh > enclosure-design/build/v15-r1-project-check.log 2>&1
nix develop .#enclosure -c python enclosure-design/package_redesign.py
```

外壳专用环境从项目现有 `flake.lock` 取 OpenSCAD、Manifold、Trimesh 和 PrusaSlicer，
不加入固件依赖，不改工具链锁定版本。机械检查独立于默认 CI，两者都须通过才能打包。

| 正式入口的缺失对照 | 预期结果 |
| --- | --- |
| `--without-front-stop` | 前止挡拦截失败，移除止挡的放行对照仍通过，退出 1 |
| `--without-usb-rounds` | 左右 USB 槽底补料检查失败，其余前置检查通过，退出 1 |
| `--without-load-roots` | 四件补料、19 处连接及前角放大干涉对照失败；旧芯部检查仍通过，退出 1 |
| `--without-mesh-cleanup` | 四件 STL 导出行拒绝零面积面或精度塌陷，退出 1 |
| `--without-precision-cleanup` | 保留前段简化但跳过短边合并，四件 STL 导出行失败，退出 1 |

`checks/` 中的缺失对照为故意失败的检查；正常总结果以 `geometry-and-meshes.json` 为准。
打印文件通过几何检查不替代实物载荷和疲劳验证。
