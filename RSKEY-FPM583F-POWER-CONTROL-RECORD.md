<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# FPM583F 按需供电与触摸 UP 执行记录

2026-10-13。依据[修改计划](RSKEY-FPM583F-POWER-CONTROL-PLAN.md)实施；软件离线范围已验收，原始完整 gate 退出 0。实机与交付配置未验收，历史阶段 1–5 的日志和哈希不证明本次修改。

## 板级候选与 P1 边界

用户本轮确认按 GP2/GP3 候选推进软件：GP0 TX、GP1 RX 为 UART0，GP2 为无内部拉电阻的高脉冲 TOUCHOUT，GP3 为低电平开启的 VCC 开关；BOOTSEL 保留备用确认。`FINGERPRINT_POWER_PIN=3 FINGERPRINT_TOUCH_PIN=2` 显式启用；不设置两项时保留常供电与按钮对照。

V_TOUCH 拟常供 3.3V，VCC 经外部开关；AO3401A、4.7kΩ G-S 上拉、1kΩ GPIO 串联电阻仍为电路候选，尚无器件/接线、峰值与压降、UART 外拉供电来源、倒灌、A2 E9 漏电或重复触摸波形的实物结论。不能以编译和 GPIO 停用源码代替测量，也不能将脉冲低电平解释为物理离手。

普通休眠已按本地通用手册 p34–35 核对：`02 0C`、MODE=0，发送及无载荷成功应答均有独立完整向量。原件 SHA256 与[资料索引](RSKEY-FPM583F-REFERENCES.md)一致。未使用 MODE=1、RESET 引脚或其他型号的深睡路线。

## 实现与共享契约

- [供电模型](crates/rsk-device/src/fingerprint_power.rs)区分 Off、Starting、Running、Stopping 与故障 Unavailable。业务持有和未收尾操作都禁止正常休眠；故障不自动切电、重启或重发写操作。
- [串行驱动](crates/rsk-device/src/fingerprint_driver.rs)的准备包含有界启动与已有启动 Count 核对；同一请求的启动、库存及匹配共享截止时间。正常断电可准备，能力和持久配置继续可读。操作代次不会因电源周期重置。
- [板级 UART](firmware/src/fingerprint.rs)唯一持有 UART0、GP0/GP1、一次初始化的静态缓冲和可选电源输出。停用时屏蔽 IRQ、释放两个缓冲句柄、保持 UART reset、清 pending IRQ，再将引脚切为无拉电阻输入并释放到 NULL/input-disabled，最后关 VCC。恢复沿用相同所有者及缓冲并检查 reset-done。
- [触摸窗口](crates/rsk-device/src/touch.rs)与[高优先级观察任务](firmware/src/fingerprint_touch.rs)只记录当前窗口的新事件：启用后先观察低电平，再接收新上升沿，单次消费。旧代次、持续高电平、窗口外和重复事件不留给下一请求；中断执行器不操作模块供电、匹配或 Flash。
- FIDO 的准备、库存与重试仍在既有授权路径；启动失败不扣生物尝试，匹配前耐久预扣。指纹 token 的 MAC、权限与 RP 归属在准备前验证，等待后再次验证；凭据枚举 Next 也先验证当前 walk 的归属，再核对经过时间。Save/Delete/Reset 的持久隔离和不确定结果不重发规则保持原样。
- 录入跨请求持有 VCC，最终保存、库存复核和主控授权提交完成才释放。取消、超时、token 更换/失效、连接结束与其他业务命令结束会话；GetInfo 保留会话，因为第二个标准客户端会先查询能力再跨 HID/CCID 续录。pending 记录仍用于经授权恢复。

`bcdDevice` 从实施时的 `0x09DD` 递增为 `0x09DE`。未修改持久记录格式、密码学和凭据派生，未新增依赖；两个可选编号 GPIO 的 `unsafe` 所有权已补到[审计](docs/unsafe.md)。构建脚与 `EF_PHY` 的 LED 覆写同时保护 UART、触摸、电源、备用按钮及固定 Pimoroni LED 脚。

## 离线验证

主机回归及 GP2/GP3 的 2MB 目标 Clippy、链接已通过。两项新增 Kani 0.67.0 harness 返回 `2 successfully verified harnesses, 0 failures`，只证明有界的保持/事件规则，不关闭原指纹形式 gap，也不证明并发、GPIO 或实物时序。

核对 Kani 原始 GOTO 输出时发现其 CAS 模型在比较失败时仍将 success 标志设为真。触摸转换同时检查返回的实际观测值与预期窗口，保持真实 Rust 的强 CAS 语义；证明与主机负向用例均使用生产函数，无 stub 或缩小常量。代次分配采用有界单次 CAS，无重试自旋。

| 验证 | 观察到的结果 | 本地原件 |
| --- | --- | --- |
| 原生 python-fido2 生物管理与字节协议矩阵，常供电及按需供电 | 69 passed / 2 skipped；两项仅用于开关供电故障的用例在常供电组合跳过 | [bio-clients.log](target/fpm-power-evidence/raw/bio-clients.log) |
| 原指纹标准客户端回归 | 42 passed | [fingerprint-clients.log](target/fpm-power-evidence/raw/fingerprint-clients.log) |
| 原标准客户端基线 | 24 passed | [standard-clients.log](target/fpm-power-evidence/raw/standard-clients.log) |
| 供电与触摸两项 Kani harness | 2 successfully verified harnesses, 0 failures | [kani.log](target/fpm-power-evidence/raw/kani.log) |
| 原 FPM 帧 fuzz 目标（含普通休眠编码） | 20000 runs，退出 0，未发现 crash；不是硬件或状态机覆盖证明 | [fuzz.log](target/fpm-power-evidence/raw/fuzz.log) |
| 实际候选引脚构建反例 | 9 项分别因对应配置冲突而拒绝，包含未启用 fingerprint、UART、同脚、按钮、LED 与固定 Pimoroni 脚 | [pin-collisions.log](target/fpm-power-evidence/raw/pin-collisions.log) |
| 最终三个无屏 2MB 构建及 GP2/GP3 Clippy | 退出 0，无 display/no-touch | [resources.log](target/fpm-power-evidence/raw/resources.log) |
| 原始完整 `scripts/check.sh`，Linux 显式 host target | 退出 0，末行 ALL CHECKS PASSED；设备 125 项、FIDO 737 项（另 1 项原有 ignored）、脚本 3714 项、host CLI 312 项、interop 45 项通过 | [gate.log](target/fpm-power-evidence/raw/gate.log) |

客户端使用固定 python-fido2 2.2.1、真实 FIDO 业务/目录/密码学及 RP 公钥验签。按需字节模型在掉电时清除临时采样、保留非易失库存；六样本间隙、保存后库存/提交、正常释放、错误认证参数以及启动/休眠丢应答都有实际入口覆盖。模型不代替实物 UART、触摸、USB 或模板耐久性。

原始完整 gate 从头执行并全绿，未跳过检查或降低门槛。新增源码导致的形式引用已按相同代码及授权谓词核对后更新，引用锁没有沿用错误行号；新增 Next 前置归属检查及两个 GPIO 所有权纳入原登记。相应计数、文档与反向测试基准按实际树同步；错误计数仍须被拒绝。两项 GPIO 使用不同名称表达真实所有者，原平台假设仍为 pending。

## 保护的反向验证

下列临时修改分别通过原始 `check.sh` 的 **test (host)** 行触发失败，均已读过实际断言并恢复。正向主机用例与客户端复现使用生产实现；没有删掉测试、放宽失败条件或用成功状态替代模块结果。

| 临时修改 | 实际失败及方向 | 原件 |
| --- | --- | --- |
| 触摸转换不再核对原窗口和状态 | 重复 `record(first)` 意外为 true，`!touch.record(first)` 失败，gate 退出 101 | [mutation-pulse-state.log](target/fpm-power-evidence/raw/mutation-pulse-state.log) |
| 保留状态但忽略触摸代次 | 旧窗口意外消费新入口的脉冲；“an old window must not consume the new transport's pulse” 失败，gate 退出 101 | [mutation-pulse-owner.log](target/fpm-power-evidence/raw/mutation-pulse-owner.log) |
| 关闭供电时忽略业务持有 | 当前采样/会话仍持有时 cuts 为 1，预期为 0；设备驱动与状态模型共 3 项失败，gate 退出 101 | [mutation-power-hold.log](target/fpm-power-evidence/raw/mutation-power-hold.log) |
| 恢复原先先库存、后认证的五个 token 消费入口 | 错误认证仍返回 PinAuthInvalid，但 prepare 从 7 增至 8，副作用断言失败，gate 退出 101；SDK 的两种供电组合也因额外上电/库存失败 | [mutation-auth-order.log](target/fpm-power-evidence/raw/mutation-auth-order.log)、[客户端复现](target/fpm-power-evidence/raw/auth-order-clients-repro.log) |

启动/休眠应答恰在截止时间完成的测试是正向边界覆盖：现有 Exchange 分步轮询正确拒绝该应答，未将一个通过的用例写成失败复现。恢复后的触摸及供电模型源码 SHA256 分别为 `22f964c35d216bea9a232c1d641317188a8dc63875651ac0a265a841ebaf138a` 与 `184ab988f95e6eb86858f162b3b7c2e20de59cd9db96bfc4068bdb84ce5dfbab`。

## 最终无屏 2MB 资源

均为 `FLASH_SIZE=2097152 KVMAIN=896K`、`--features fingerprint`，其余采用默认板级设置；代码/main/counter 分区保持 1MiB/896KiB/128KiB。使用独立 `CARGO_TARGET_DIR=target/fpm-power-build`，避免覆盖完整 gate 的测试输出。

| 项目 | 常供电 + 按钮 | 常供电 + GP2 触摸 | GP3 按需供电 + GP2 触摸 |
| --- | --- | --- | --- |
| GNU size text（含 RAM 初始化数据） | 845212B | 847212B | 846996B |
| 静态 RAM | 344268B | 344364B | 344364B |
| Core0 栈区域 | 180000B | 179904B | 179904B |
| 距 168KiB 下限 | 7968B | 7872B | 7872B |

静态 RAM 取 `__ebss - __sdata`，栈取 `_stack_start - _stack_end`；按需组合比阶段五的静态 RAM 增加 128B。真实栈高水位、功耗和延迟尚未测量。构建与符号核算可由[资源脚本](target/fpm-power-evidence/raw/build-resources.sh)复现，原件保存在本地忽略的 `target/fpm-power-evidence/`。

| raw ELF | SHA256 |
| --- | --- |
| [constant-button-2mb.elf](target/fpm-power-evidence/constant-button-2mb.elf) | `9953dbbe39afe7afb9a9c9e02f43ff5d1e317ce052d83e0fedcdcebbe5b3cfde` |
| [constant-touch-2mb.elf](target/fpm-power-evidence/constant-touch-2mb.elf) | `e1866c93a150c7f33647da4c3ccdea8ccc4ded283af2056be6d6930375d7d9f9` |
| [switched-2mb.elf](target/fpm-power-evidence/switched-2mb.elf) | `966a9550fbc5f5bd041345ec4b98fa013077ba9b0a6de3cb59616d435b258596` |

这些 ELF 未应用分区表，只作资源审阅，不是已验收的可部署镜像。完整 gate 留下的是 no-touch 测试镜像；需要交付时须另按构建流程生成并验收目标配置。

最终 gate 原件 SHA256 为 `554e1de8c2cb90ca25a287f0631481e651c91b78527b7ba3a5aeacd6ef323e80`。[本地哈希清单](target/fpm-power-evidence/SHA256SUMS)登记 22 项镜像、脚本及原始日志，清单自身 SHA256 为 `a0b0e865acfb42506441d3d8bdb9e8be033860787021267c4a0d3d21a895ac22`；文档完成状态回写后的链接检查另保存在 `raw/docs.log`。

## 复现

```sh
export NIX_CONFIG='experimental-features = nix-command flakes'
nix develop --no-update-lock-file -c bash scripts/bio-client.sh
nix develop --no-update-lock-file -c bash scripts/fingerprint-client.sh
nix develop --no-update-lock-file -c bash scripts/standard-client.sh
nix develop --no-update-lock-file -c env HOST_TARGET=x86_64-unknown-linux-gnu ./scripts/check.sh
nix develop --no-update-lock-file -c bash target/fpm-power-evidence/raw/build-resources.sh
nix develop .#fuzz --no-update-lock-file -c cargo fuzz run fpm_frames -- -runs=20000 -max_len=2048
```

Kani 使用本地固定 0.67.0 入口、`rsk-device --features kani-soft` 下的 `a_hold_or_undrained_operation_prevents_shutdown` 和 `a_closed_or_consumed_pulse_cannot_confirm_a_new_window`。根 Cargo、flake 与模拟器锁定文件 SHA256 依次保持 `e0719d516ac008462724a853d57b5c0fb25425d1de72e2a19966c9a7f0912481`、`329b2d37efcae49a4dec8a14d28a8257304d999f78f6cfdd4e219b7d944b281b`、`7fb1e8eba983b8eaeb8a509238fb9498f081725b9a57e4bf9748e05230f7adf0`。

## 未验收与操作边界

P1 目前只完成资料/源码审查与候选配置；P2–P4 软件已实现，P5 的软件回归、实际 gate 反向验证与最终资源汇总完成。V_TOUCH 常供且 VCC 关闭时能否反复产生新的独立触摸，以及普通休眠后断电和真实模板耐久性，都仍须步骤 6 验收。步骤 7 的真实 USB、浏览器、第二客户端和三平台体验未完成。

本轮未连接、刷写、复位、擦除、生产签名或写入 OTP。最终交付配置须遵循[安全规则](SAFETY.md)，实测方法与门槛按修改计划第 6 节固定后实施。
