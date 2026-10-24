<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# 自动重新休眠候选 09EE：重复触摸通过，长按恢复未通过

日期：2026-10-23，Asia/Shanghai。用户明确授权刷写并复测固定 `09EE` 镜像，
限 ROM 序列号 `2E64C924B24D1A5E` 的代码分区 0；不清库、不改 PIN、不签名、不写 OTP。
本轮接续 [09ED 失败记录](RSKEY-FPM583F-AUTO-REARM-HARDWARE-RECORD-2026-10-23.md)。

## 软件与部署证据

`09EE` 在维护期间忽略 GPIO 边沿，关电完成后等待低电平，再允许新上升沿确认。
150 项设备库测试、更新的 Kani harness 和完整 `check.sh` 门禁通过。门禁包含
3797 项脚本测试、322 项 CLI 测试与 46 项互操作测试；原始日志与源文件哈希见
[软件检查点](target/touch-auto-rearm-validation-09ee-2026-10-23/checkpoint.json)和
[完整门禁日志](target/touch-auto-rearm-validation-09ee-2026-10-23/gate.log)。
软件绿灯不能代替下面的硬件结果。

镜像 SHA-256：`cbcf9d395308f5e1602d18ce52f63729926b8c795a25ffc20f41b9a9a4f900ab`。
刷写前核对 secure-boot、分区边界及旧 `09ED` 镜像。限定代码分区的 load 和随后
独立 verify 均退出 0，重启后的应用实际枚举为 `09EE`、`1209:0001`、
`rs-key-0001`。命令只写代码分区，未执行存储擦除、PIN 修改、签名或 OTP 写入。
GetInfo 的 `clientPin` 刷写前后均为 false，不证明已有凭据或模块模板的内容。

## 实机结果

使用 `authenticatorSelection`（`0x0B`）测试确认，不创建凭据。
每次新触摸前静置 7 秒；确认请求报告 UPNEEDED 后至少静置 1 秒，才弹窗提示触摸。
成功必须发生在提示后，且操作员确认仅触摸模块、未按 BOOTSEL、已移开手指。

| 项目 | 结果 | 证据范围 |
| --- | --- | --- |
| 同一通道连续触摸 10 次 | 10/10 成功，CTAP `0x00` | 各轮操作员确认；中间未重启、未手动重新休眠 |
| 空闲时触摸，随后新请求重新触摸 | 成功，CTAP `0x00` | 新请求仍要求提示后的触摸 |
| 窗口打开前保持触摸，提示后松手、静置 7 秒并重新触摸 | 失败，约 30.33 秒返回 `0x2F` | 用户确认完全按要求操作，仍未响应 |
| 取消 | 成功，主机发送 CANCEL 后返回 `0x2D` | 长按失效后单独测试，约 1.22 秒完成 |
| 无输入超时 | 成功，约 30.01 秒返回 `0x2F` | 长按失效后单独测试，保持两个输入不操作 |
| BOOTSEL 备用确认 | 成功，CTAP `0x00` | 长按失效后单独测试；操作员确认仅按按钮 |
| 取消／正常超时后分别重新触摸 | 未执行 | 不等同于上述控制路径通过；长按后的独立触摸恢复已失败 |

前 11 个成功请求前共采集 110 次状态，全部为关闭、无 UART／驱动故障。
长按失败后另采集 100 次只读状态，也全部为关闭、无 UART／驱动故障。
这些状态不等于直接测量 VCC 或 GP2 波形；本轮没有观察到 `09ED` 的反复启动，
但不能据此解释长按失败的唯一原因。

长按失败后，另起一个独立的新请求做恢复对照：操作员先完全松手，等待 7 秒，
10 次状态采样均正常，再等新提示触摸。该请求仍约 30.01 秒返回 `0x2F`；用户确认
提示后确实重新触摸并移开，设备未响应。原失败日志与这次补充对照分开保存。
因此问题影响后续触摸，不能只归于第一次长按的当前窗口。

BOOTSEL、取消及正常超时的独立对照均通过；末尾 10 次状态采样仍全部为关闭、
无 UART／驱动故障。补充对照没有改变前面长按及新请求触摸恢复失败的结论。

## 限制与当前状态

板上保留 `09EE`，未自动重刷。本轮证明了以上 11 次成功触摸确认，同时记录长按恢复失败，
不能标作全部实机验收通过。尚未验证实际认证数据的 UP／UV 标志、指纹匹配、
凭据／模板内容保持、断电恢复、长期静置或不同供电条件。

下一步应先同步记录长按前后的 GP2 电平、边沿和控制器阶段，定位未生成新边沿与
未重新武装之间的区别。现有六字节状态不包含 GP2 或控制器阶段，不能仅凭“关闭”
推断唯一原因；也不能把操作员已确认的失败排除为误操作。

## 本地证据

- [部署命令与独立读回](target/touch-auto-rearm-candidate-09ee-2026-10-23/install-events.log)
- [原实机请求、状态及操作员确认](target/touch-auto-rearm-candidate-09ee-2026-10-23/acceptance-events.jsonl)
- [原实机输出及失败断言](target/touch-auto-rearm-candidate-09ee-2026-10-23/acceptance.log)
- [失败后只读采样](target/touch-auto-rearm-candidate-09ee-2026-10-23/status-after-held-failure.log)
- [补充恢复对照输出](target/touch-auto-rearm-candidate-09ee-2026-10-23/followup.log)
- [备用控制路径对照输出](target/touch-auto-rearm-candidate-09ee-2026-10-23/fallback-followup.log)
- [汇总、最终 USB 身份及证据哈希](target/touch-auto-rearm-candidate-09ee-2026-10-23/hardware-analysis.json)
- [会话检查点](target/touch-auto-rearm-candidate-09ee-2026-10-23/SESSION.json)

原始日志位于本机忽略目录 `target/`；本记录不表示这些日志已加入版本控制。
