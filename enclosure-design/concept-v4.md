<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# 主板几何参考 V4

状态：已导入官方 STEP 的结构讨论稿，未达到加工或完整试装条件。
壳体更新为 51.8 × 26 × 19 mm 候选包络、两颗底部螺丝和嵌件盲孔。
19 mm 来自用户估计的板件总厚度 14 mm、上下各 0.9 mm 净空及各 1.6 mm 壁厚。

## 已落实的几何

`board_reference.py` 使用 OpenCascade 7.9.3 读取已校验的官方 STEP，
组合装配变换后提取器件外廓及按键顶面，并生成主板参考 STL。
这与 FreeCAD 使用同一几何内核，不需要安装 FreeCAD 桌面程序。
源码没有复制厂商模型内容，第三方模型保存在忽略目录。

当前模型有 104 个装配/零件节点，BRep 有效性检查通过，网格状态标志为 0。
它不是完整的实物模型：未包含 USB 插头，也未包含原理图中的 U1 Flash，
更不包含用户焊接的排针、转接板和线束。
不得把模型最高点当作当前实物最高点。

本壳坐标 X 沿 USB 到指纹模块方向，Y 为横向，Z 从底壳外底面向上。
用户补充 USB 超出板边 8 mm、主板裸 PCB 上表面至整套最低针脚 11 mm。
“同一面”在这里仅用于识别 PCB 朝向，没有据此把 PCB、USB 与指纹检测面设成共面。
主板前边改放在 X = 0.8 mm，横向居中。
STEP 原坐标到本壳坐标的变换为：

```text
X壳 = 0.8 + 23.5 − Y_STEP
Y壳 = X_STEP − 9
Z壳 = Z主板元件面 + Z_STEP
```

| 部位 | 壳体 X / mm | 壳体 Y / mm | STEP 相对主板元件面的 Z / mm |
| --- | --- | --- | --- |
| Key1 / BOOT 顶面中心 | 16.750085 | −4.499209 | 2.500000 |
| Key2 / RUN 顶面中心 | 16.750085 | 4.508695 | 2.500000 |
| L2 / RGB 封装平面中心 | 16.264410 | 0.000000 | 外廓 0～1.000000 |

BOOT、RUN 和 L2 的身份已与[微雪官方原理图](https://github.com/waveshareteam/RP2350-Zero-CM/blob/main/hardware/schematics/RP2350-Zero-CM.pdf)核对。
按键位置取自顶面几何，LED 位置取自封装外廓中心，未声称是光学中心。
V4 的检修孔及观察孔已替换为这些平面位置；孔径仍是设计选择。

主板元件面按用户数据放在 Z = 2.5 + 11 = 13.5 mm。
这使 STEP 中的已知按键顶面位于 Z = 16.0 mm，
比 14 mm 叠层占位的顶部 Z = 16.5 mm 低 0.5 mm。
11 mm 为用户报告，没有独立复测。它可确定主板面基准，
仍不能给出转接板承托面、焊点避让区或 USB 轴线；PCB 压条和按钮帽尚未完成。

## USB 与固定结构的限制

官方 STEP 缺少 USB，当前取消了 V4 中虚构的金属插头占位。
前壳暂开宽 9 mm、高 14.6 mm 的观察口，用于试插定位，不能当作最终 USB 开孔。
V1～V3 仍保留各自的历史占位值。

旧 X = 7 mm 的后缩量会使 8 mm 插头只外露 1 mm，已经弃用。
用户明确轴向阻挡实测不得超过 1 mm；此前 1.2 mm 前壁版本超限，已作废。
当前按名义 0.8 mm 设计，给打印、装配与板件前后定位的合计偏差留 0.2 mm。
阻挡按 PCB 前边到壳体最前部计算，侧壁、圆角、凸台也不能向前越过该基准。
预留值不代表加工商能保证的公差；超出 1 mm 的实物不能验收，不能用试插成功代替这项尺寸要求。

V4 移除两颗前端螺柱，将剩余两颗放在 (28.6, ±10) mm，
PCB 和模块相对 V3 整体前移 6.2 mm，模块中心 X = 38.8 mm。
前端局部直壁厚 0.8 mm，其内侧作为裸 PCB 前缘的定位面；其他主体壁厚保留 1.6 mm。
0.8 mm 局部薄壁的加工及强度仍需试样核对。
PCB 宽度范围内另做前端避让腔，避免前移后板框穿入原有 1.6 mm 壳壁。

按用户要求采用标准 Type-C 金属配合段宽 8.25 ± 0.03 mm、厚 2.40 ± 0.03 mm，
依据 [USB-IF Release 2.5](https://usb.org/document-library/usb-type-cr-cable-and-connector-specification-release-25)
第 66 页图 3-12；图中轴向配合段为 6.65 ± 0.10 mm。
本地 PDF 存在 `build/reference/USB-Type-C-R2.5.pdf`。
用户的 8 mm 减去 0.8 mm 名义阻挡得到 7.2 mm 名义外露。
在伸出量确为 8 mm 的前提下，1 mm 阻挡上限对应至少 7 mm 外露。
标准只定义配合段，不定义这块板的焊接高度、根部外廓或目标设备外壳凹入量。
预览中的 USB 俯视线框是标准宽度与用户伸出长度的组合示意，不是 STEP 内的器件。

现有底壳的 PCB 侧隔壁及指纹承托仍为候选结构。
PCB 支撑、可拆压条、模块压环、无卡扣定位止口和线束槽尚未定稿。
两颗螺丝布局的上盖刚度、接缝及抗扭尚需实物检查；若不足，需调整壁筋或局部宽度。
金属螺丝及嵌件仍放在板框外，保留盲孔和塑料间隔的几何约束；
这些约束不等于完成实物绝缘验证。

## 生成与复核

先按[尺寸记录](dimensions.md)中的官方来源，将已校验 STEP 放在
`build/reference/RP2350-Zero-CM.step`。本机 CAD 工具不属于固件依赖，没有修改项目 flake。
`OCCT_ROOT` 应指向已有 OCCT 7.9 安装目录，包含 `include/opencascade` 和 `lib`。

```sh
nix develop -c python enclosure-design/board_reference.py --occt-root "$OCCT_ROOT"
nix develop -c python enclosure-design/model.py --variant v4
nix develop -c python enclosure-design/reference_preview.py
```

输出为 `build/reference/board-geometry.json`、`build/reference/board.stl`、
`build/concept-v4.scad` 和 `build/concept-v4-review.png`。
图中俯视主板投影来自官方网格；指纹模块和侧面叠层为示意。
底壳/上盖可分别以 `part="base"` / `part="lid"` 导出检查。
`part="envelope-collision"` 检查壳体与 14 mm 板件占位的相交。
`part="front-gauge"` 导出 6 × 26 × 19 mm 一体式前端试插规，
两侧小刻线中心对应 Z = 13.5 mm 主板面。它用于先试接口长度及鼻部避让，不包含最终 USB 小孔。
修订版导出为 `build/v4-front-gauge-usb0p8.stl`，并同步更新旧路径 `build/v4-front-gauge.stl`。
以毫米导入，先核对 6 × 26 × 19 mm 外廓；此前下载的 1.2 mm 版本不得用于此次要求。
它可作为第一件 PA12 试插样片；不用先打印整壳，也不包含螺丝或卡扣。
`preview.py --gauge` 可从同名 OFF 检查网格生成 `build/v4-front-gauge-preview.svg`。
当前生成的 PNG 可直接查看。板件前移后的整体核对图为 `build/concept-v4-review.png`。
这些输出均为本机讨论与几何检查文件，不是加工交付物。

已观察到的检查结果：底壳与上盖均为简单有效实体，壳体与板件占位相交结果为空。
将占位单独向 USB 方向移动 3 mm 后，相交结果非空，验证检查能检出前端壁面干涉。
对应日志及当前摘要在 `build/v4-*-check.log`、`build/v4-usb0p8-geometry-checks.json`。
上下壳网格的最前端均为 X = 0 mm，配合 PCB 前边 X = 0.8 mm，名义阻挡核对为 0.8 mm。
将 CAD 参数改回 `pcb_x=1.2` 时，导出因 USB 阻挡约束而失败；没有把其他几何错误当作超限证据。
试插规实体检查同样通过。这些结果不验证实物尺寸、固定受力、USB 配合或金属绝缘。

上一版完整项目门禁已通过，记录为 `build/v4-project-check.log`。
本次 USB 阻挡修订的完整门禁也已通过，以 0 退出并输出 `ALL CHECKS PASSED`，
记录为 `build/v4-usb0p8-project-check.log`。
软件门禁通过不替代实物试插与绝缘检查。

## 下一步

先实测前端试插规的轴向阻挡不超过 1 mm，再检查插头能否插到底、前端裸板边能否接触塑料定位面及针脚避让。
试插规需手持定位，使裸 PCB 上表面对齐两侧刻线，不能把它当作已经固定板件的外壳。
完成后记录 USB 金属外壳相对主板面的中心高度及根部外廓，再收紧前端开孔。
再完成 PCB 压条、模块压环和线束限位，最后导出整壳试装件。
螺丝长度在嵌件规格及实物高度确认后选定。
