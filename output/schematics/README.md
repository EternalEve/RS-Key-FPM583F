<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# RP2350-Zero-CM 与 FPM583F 接口原理图

Rev A，2026-10-13。接线参考图，未实物验证，尚不是可直接投产的 PCB / Gerber 文件。

- [SVG 矢量原图](rp2350-zero-cm-fpm583f-rev-a.svg)：可以放大查看和编辑。
- [PNG 预览](rp2350-zero-cm-fpm583f-rev-a.png)：2600 × 1920。

GPIO 分配采用当前[电源控制工程记录](../../RSKEY-FPM583F-POWER-CONTROL-RECORD.md)已确认的 GP0 / GP1 / GP2 / GP3。

| 微雪板接口 | 官方原理图 P1 编号 | 经接口板连接到 |
|---|---|---|
| VBUS / 5V | 23 | XC6206P332MR-G 的 VIN（3 脚） |
| GND | 22 | 公共地、LDO VSS（1 脚）、模块 GND（6 脚） |
| GPIO0 / GP0，UART0 TX | 1 | 模块 RX（5 脚） |
| GPIO1 / GP1，UART0 RX | 2 | 模块 TX（4 脚）；10kΩ 上拉到受控 VCC |
| GPIO2 / GP2 | 3 | 模块 TOUCHOUT（2 脚） |
| GPIO3 / GP3 | 4 | 经 1kΩ 串联电阻驱动 AO3401A 的 G（1 脚） |
| 板上 3V3 | 21 | 本设计不连接，避免与独立 LDO 的输出并联 |

P1 编号取自微雪 CM 版官方原理图的 `Header 23`，不是 RP2350 芯片管脚号。按 GPIO / VBUS / GND 标签定位板上焊盘，不能将这张逻辑连接图当作板子正反面的物理排布图。

模块按规格书第 6 节接口定义：1=V_TOUCH、2=TOUCHOUT、3=VCC、4=TX、5=RX、6=GND。V_TOUCH 接独立 LDO 的常供 3.3V；VCC 经 AO3401A 切换。模块手册第 9 节参考电路的连接器符号为反序，不能直接照抄该符号的编号到新板焊盘。

图中 U1 为 XC6206P332MR-G（SOT-23，固定 3.3V），1=VSS、2=VOUT、3=VIN；Q1 为 AO3401A（SOT-23），1=G、2=S、3=D。两者虽同为三脚 SOT-23，管脚功能并不相同。

R1=4.7kΩ G-S 上拉、R2=1kΩ GPIO 串联是工程记录的候选选择；模块原参考图 R1 为 100kΩ。GP3 低电平开启、高电平关闭。C1、C2 各 1µF，与手册独立 LDO 参考电路一致，图中标出的陶瓷介质和耐压是本图选型建议。

模块 TX → MCU RX 线路的 R3=10kΩ 上拉接受控 `3V3_FP`；断电前固件仍必须将 UART 双脚设为无上下拉的高阻状态，RP2350 A2 还须处理 E9 输入缓冲漏电。GP2 只为当前 UP 提供触摸事件；UV、模板管理等模块任务按需开启 VCC。

D1-D5 采用模块手册图 8 推荐的 PESD5V0S1BL，分别从 V_TOUCH、TOUCHOUT、VCC、TX、RX 并联到地，靠近模块连接器；1=信号，2=GND，器件为双向结构。该推荐型号工作反向电压为 5V，其钳位特性与 RP2350 的实际保护配合仍须在硬件设计时审核。R4 仅为可选 TOUCHOUT 外部下拉焊盘，默认不装（DNP），具体值需结合芯片修订版、A2 E9 和模块输出驱动能力确认。

制作 PCB 前还需确认：模块实物 1.00mm / 6 pin 连接器的完整型号、朝向和排线映射；微雪 CM 板的实物焊盘及安装尺寸；LDO 的电流与散热裕量；主电源峰值与压降、断电倒灌及 V_TOUCH 常供下的重复触摸波形。当前图不声明这些实物条件已通过验收。

依据：

- [微雪 RP2350-Zero-CM 官方资料](https://docs.waveshare.com/RP2350-Zero-CM)，及仓库保存的[官方原理图快照](../../RSKEY-FPM583F-PHASE-1-EVIDENCE/sources/RP2350-Zero-CM.pdf)。
- [FPM583F V1.0.2](../../FPM583F.pdf)，第 6 节和第 9 节图 5-8。
- [Torex XC6206 数据手册](https://product.torexsemi.com/system/files/series/xc6206.pdf)。
- [AOS AO3401A 数据手册](https://www.aosmd.com/sites/default/files/res/data_sheets/AO3401A.pdf)。
- [Nexperia PESD5V0S1BL 数据手册](https://assets.nexperia.com/documents/data-sheet/PESD5V0S1BL.pdf)。
