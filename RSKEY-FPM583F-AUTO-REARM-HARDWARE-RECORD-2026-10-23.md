<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# 自动重新休眠候选 09ED 实机验收：未通过

日期：2026-10-23，Asia/Shanghai。用户明确授权代理刷写指定 `09ED` 镜像并验收，
限 ROM 序列号 `2E64C924B24D1A5E` 的代码分区 0；不清库、不改 PIN、不签名、不写 OTP。
本记录接续[软件验证快照](RSKEY-FPM583F-AUTO-REARM-VALIDATION-2026-10-23.md)。

## 部署与测试结果

旧 `09F1` 镜像、secure-boot 状态和现有分区均核对通过；限定代码区的加载工具仅对
分区 0 执行 load，随后通过独立 verify，应用枚举为 `09ED`。未执行存储擦除或 OTP
写入。刷写前后 GetInfo 的 `clientPin` 都为 false；本轮不证明已有凭据或模块模板的内容。

镜像 SHA-256：`998eb62211abf776a5eba3970a084e264d6044794dec478f63898763d860a2d0`。
部署退出码为 0，原始命令与返回见部署日志（本地文件：`target/touch-auto-rearm-candidate-09ed-2026-10-23/install-events.log`）。

使用现有 `authenticatorSelection` 确认测试，不创建凭据或改变 PIN。每轮前保持松手
并等待 7 秒，看到 UPNEEDED 后至少静置 1 秒才提示操作；提示前完成的确认会被拒绝。

| 请求 | 结果 | 操作员信息 |
| --- | --- | --- |
| 第 1 次触摸确认 | 成功，CTAP `0x00` | 弹窗确认仅触摸模块并移开，未按 BOOTSEL |
| 第 2 次触摸确认 | 超时，CTAP `0x2F` | 用户报告“仅第一次触摸正常” |

第二次失败后流程立即停止，未自动重试刷写。后续连续十轮、空闲触摸、取消／超时恢复、
BOOTSEL 备份均未执行，不能标作通过。本测试只观察确认结果，不返回认证数据中的
UP／UV 标志，也没有验证指纹匹配。

## 故障观察与软件复现

失败后的只读 `INS 0x15` 状态采样包含 100 个样本，跨度约 5.289 秒：88 个报告
供电启动中，12 个报告 UART 就绪；至少观察到 12 次 UART 就绪→供电启动的转换。
各样本均无 UART／驱动故障记录。用户确认该采样期间一直未碰模块或 BOOTSEL，
手指已移开。它证明适配器反复进入上电流程，不是独立测得 VCC 电压或 GP2 波形。

原状态机将维护期间的高低边沿记为下一次维护，存在电源动作反复触发维护的路径。
这是结合代码与状态采样的机制判断，尚未采集电源切换时 GP2 的精确波形。
新增主机测试模拟维护期间高低边沿，原代码启动 10 次，断言只允许 2 次，退出 101；
失败方向是多余启动，见复现日志（本地文件：`target/touch-auto-rearm-validation-09ee-2026-10-23/repro.log`）。

修正候选 `09EE` 忽略维护期间的边沿，成功关电后先等待低电平，再接受新的上升沿；
高电平或旧窗口不能提供确认。当前 150 项设备库测试通过；其完整门禁和实机复测
须分别验证，不继承 `09ED` 的绿灯。

新保护已通过实际 `check.sh` 的 `test (host)` 行做故障注入；临时副本未过滤测试名。

| 临时副本 | 门禁行退出码 | 已核对的结果 |
| --- | ---: | --- |
| 正常修正基线 | 0 | 完整工作区主机测试通过 |
| 令 Running 期间的高边沿重新进入 High | 101 | 回归断言报告启动 8 次、要求 2 次；故障方向为多余上电 |

原始结果（本地文件：`target/touch-auto-rearm-validation-09ee-2026-10-23/mutations.json`）与
执行脚本（本地文件：`target/touch-auto-rearm-validation-09ee-2026-10-23/mutation-runner.py`）保留了完整门禁行。

用户已拔下 USB，停止反复上电。板上仍保存 `09ED`，当前断电；不得把本轮失败写成修复完成。

## 本地证据

- 测试与操作员事件（本地文件：`target/touch-auto-rearm-candidate-09ed-2026-10-23/acceptance-events.jsonl`）
- 测试输出及失败断言（本地文件：`target/touch-auto-rearm-candidate-09ed-2026-10-23/acceptance.log`）
- 失败后状态采样（本地文件：`target/touch-auto-rearm-candidate-09ed-2026-10-23/status-sampling-after-failure.log`）
- 会话检查点（本地文件：`target/touch-auto-rearm-candidate-09ed-2026-10-23/SESSION.json`）

首次基线读取曾因主机 HID 读取超时而退出，未刷写。改用前轮已验证的定点 hidraw
通道后，只读实测成功，34 项模拟测试通过，再执行上述部署；原失败日志保留在
`target/touch-auto-rearm-candidate-09ed-2026-10-23/baseline-attempt-1/`。
