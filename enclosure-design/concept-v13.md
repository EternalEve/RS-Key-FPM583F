<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# V13：USB 槽底两角圆角化

本页保留 V13 的历史记录；后续 [V14 完整打印稿](concept-v14.md) 已处理下文记录的网格导出问题。
当前生成器和检查入口输出 V14，复现 V13 使用 `build/v13-source-preserved.zip` 中的源码。

2026-11-02。沿用 V12 的深底壳、浅上盖和左右托架，仅优化底壳 USB 槽底的两个内角。
用户要求保留结构强度；本次以不减原有承力截面、保住 USB 净空为几何约束。
圆角消除理想尖角，但未做载荷仿真或实物强度比较，不能据此保证整机强度或寿命。

## 几何改动

两个槽底内角由直角改为 **R0.4 mm、与槽底及侧壁相切的圆弧**，
贯穿前壁完整的 0.8 mm 厚度。圆角通过在原开口角落补料形成，不向槽底开卸荷孔。
原有底壳的材料集合完整保留；上盖、左右托架、PCB 接触面及紧固件位置未调整。

| 项目 | 名义值 |
| --- | --- |
| 最大外形，不含外露 USB | 51.8 × 28 × 19 mm，与 V12 相同 |
| USB 轴向阻挡 | 0.8 mm；仍留 0.2 mm 给原有总偏差预算 |
| 开口两侧直边间距 | 9.2 mm |
| 槽底标高 | Z = 13.25 mm；圆角处不再是完整矩形开口 |
| 槽底圆角 | 两处 R0.4 mm |
| USB 参考根部 | 宽 8.25、高 3 mm，下缘 Z = 13.5 mm |
| 最小 USB 名义净空 | 保留 0.25 mm；另以圆形外扩包络检查角落净距 |

圆角半径不是越大越好。现有单侧横向余量为 `(9.2−8.25)/2 = 0.475 mm`，
R0.4 的槽底平段半宽为 `4.6−0.4 = 4.2 mm`，仍覆盖 USB 半宽 4.125 mm，
因而 USB 下方的 0.25 mm 净空不受圆角侵入。几何检查另外验证四周 0.25 mm 等距外扩的包络。
R0.8 对照仍能容纳裸 USB 包络，但会侵入这部分净空，因此不采用。

这项优化不减小外形尺寸，不以减少塑料实体体积作为指标。
不依赖削薄侧壁、槽底或扩大 PCB 承力面的让位来换取圆角。
前槽仍向上开放，沿用原板件及托架整体下落、随后竖直合盖的装配方式。

## 复核入口

在已安装 OpenSCAD 的环境中设置 `OPENSCAD` 为其可执行文件路径：

```sh
nix develop -c python enclosure-design/verify_redesign.py --openscad "$OPENSCAD"
nix develop -c python enclosure-design/usb_slot_preview.py
HOST_TARGET=x86_64-unknown-linux-gnu nix develop -c ./scripts/check.sh > enclosure-design/build/v13-project-check.log 2>&1
```

机械专用入口仍独立于项目 CI。它保留 V12 的实体、干涉、限位及装配路径检查，
增加两侧圆角补料、原截面保留、圆弧内留空、0.25 mm USB 外扩净空和故意偏移的检查。
R0.8 对照必须报告侵入外扩包络的正碰撞体积，不能以导出或断言错误代替。

缺失圆角对照由同一正式入口运行：

```sh
nix develop -c python enclosure-design/verify_redesign.py --openscad "$OPENSCAD" --without-usb-rounds
nix develop -c python enclosure-design/verify_redesign.py --openscad "$OPENSCAD" --without-front-stop
```

| 运行 | 应出现的结果 |
| --- | --- |
| 正常 V13 | 全部检查通过 |
| `--without-usb-rounds` | 左、右圆角补料检查都失败，因目标补料为空而退出 1；其余前置检查通过 |
| `--without-front-stop` | 前止挡拦截检查失败，移除止挡的放行对照仍通过，退出 1 |

结果记录在 `build/v13-geometry-checks.json` 及两个分别命名的缺失对照 JSON 中。
CAD 为 `build/concept-v13.scad`，局部对比图为
`build/concept-v13-usb-detail.png`。V12 已验证的直角版本及源文件保存在
`build/RS-Key-enclosure-V12-architecture-candidate.zip` 与 `build/v12-source-preserved.zip`。
不要混用不同版本的零件；CAD 评审包为 `build/RS-Key-enclosure-V13-usb-slot-CAD-review.zip`。
包内保留独立作差确认 V12 原截面未被切除的结果、两项缺失对照及项目完整门禁日志。

## STL 导出复核限制

四件名义 STL 已导出，但进一步用 OpenSCAD 重新导入并做布尔运算时，
底壳报告网格不封闭，托架也出现 CGAL 错误。冻结的 V12 原加工包底壳同样报错，
因此这不是本次槽角圆角化独有的问题。生成时的 `Simple: yes` 不能替代导出文件的重导入检查。

二进制 STL 可消除托架的重导入错误；底壳还包含共线零面积面，
仅重分三角面后仍未通过本机 CGAL 重导入，不能把这一尝试标为修复成功。
原始 CAD 的差集和装配检查通过，但目前 **STL 未通过打印网格复核，不应直接下单**。
本次评审包只包含 CAD、参数、对比图和检查记录，不附 STL；
原有传感器承托等部位的网格退化问题单独记录于 `build/v13-stl-export-limitation.json`。

## 尚需实物验证

现有 USB 根部宽度仍采用参考包络，未拿到完整实测几何；应核对焊接偏斜、
打印圆角和槽底尺寸、插头是否插到底，以及原有 0.2 mm 轴向总偏差预算。
R0.4 的形状能否按材料和工艺稳定成形也需加工方确认。
在同材料、同打印方向和相同载荷下比较 V12/V13 的槽角裂纹、前壁位移及板件位移，
才可量化强度和寿命变化；这里不宣称这两角是实物唯一或已实测最大的应力位置。
