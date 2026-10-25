<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# FPM583F 自动重新休眠候选的软件验证

日期：2026-10-23，Asia/Shanghai。本页保留 `bcdDevice 0x09ED` 的软件验证快照。
该软件验证阶段没有刷写、签名或写 OTP，当时板上为 `09F1` 隔离诊断版。后续授权
实机验收仅首次触摸成功，见[实机失败记录](RSKEY-FPM583F-AUTO-REARM-HARDWARE-RECORD-2026-10-23.md)；
本页的软件绿灯不代表实机通过。硬件对照依据见
[重新休眠实验记录](RSKEY-FPM583F-TOUCH-REARM-TEST-RECORD-2026-10-23.md)。

## 实现与边界

TOUCHOUT 持续观察；只有当前 FIDO 确认窗口内、先低后高的新输入能提供 UP。
触摸松手后，包括窗口外触摸，worker 串行执行 Count、普通 Sleep、UART 安全关闭
和主 VCC 关闭，然后重新允许新触摸。长按不反复启动模块；维护过程中出现的新触摸
仍须松手并完成维护，不能借用维护过程确认请求。

登记和匹配持有的 UART／供电优先，维护等待释放与排空。维护与正常操作共用同一个
驱动；短借用适配器允许确认等待逐步推进维护，不新增并行 UART 所有者、堆分配、
依赖或持久记录。维护不清除请求的等待／取消标志，不延长确认期限；通信或休眠失败
禁用本次启动的 GP2，不自动重新上电。BOOTSEL 与 PIN 路径保留。

`ctutils` 在根、模拟器、TUI 和 fuzz 四份锁文件中仅从 0.4.2 更新到 0.4.4，
处理 RUSTSEC-2026-0385；TUI 同步升至 0.3.13。未更新 Embassy 或工具链锁定。
已审阅 crates.io 两个版本的源码、清单与测试差异，并运行新版上游 150 项单元测试和
41 项属性测试，全部通过；在 `supply-chain/audits.toml` 记录了明确标注的自动增量
审阅，`cargo-vet` 已通过。原 0.4.2 基线豁免及部署标准保留，不宣称外部密码学审计
或 RP2350 上的恒定时间证明。

源码插入引起的形式化引用行号和派生文档已同步；TLA 可执行模型内容与原版本一致，
没有改变模型假设或证明范围。

## 软件验证

原代码的复现测试在首次触摸结束后要求第二轮 Count／Sleep，实际模块启动次数仍为
1，断言期望为 2；失败方向与重复触摸缺少重新休眠的缺陷一致。

- 最终设备库 148 项测试通过，回归覆盖连续三轮维护、模板保留、窗口外触摸、长按、维护中的触摸、
  登记／匹配持有、业务接续、通信失败禁用、取消／超时后恢复及 BOOTSEL 备份。
- 新增 Kani harness `maintenance_and_old_edges_never_grant_new_consent` 已通过。
  它验证状态机的窗口隔离，不证明实际 GPIO 时序、供电电压或并发硬件行为。
- 文档构建和离线链接检查已通过。
- 最终主机严格 Clippy 与 2 MB `fingerprint,strict-config` 严格 Clippy 通过。
- 最终完整门禁退出码为 0，日志以 `ALL CHECKS PASSED` 结束：门禁脚本 3797 项、
  CLI 322 项、互操作 46 项测试全部通过，泄漏扫描未发现泄漏。

最终完整门禁命令：

```sh
NIX_CONFIG='extra-experimental-features = nix-command flakes' nix develop -c env HOST_TARGET=x86_64-unknown-linux-gnu ./scripts/check.sh
```

完整日志（本地文件：`target/touch-auto-rearm-validation-2026-10-23/gate.log`）与
检查点（本地文件：`target/touch-auto-rearm-validation-2026-10-23/checkpoint.json`）已保存；门禁运行前后
43 个受检文件的 SHA-256 一致。该记录页只在结果确定后补录，不包含在源码快照中。
新增测试及 Kani harness 对应的数量登记和故障夹具已同步；上一轮因此产生的
失败日志（本地文件：`target/touch-auto-rearm-validation-2026-10-23/gate-fixture-failure.log`）也已保留。

以下故障注入在临时副本执行实际 `check.sh` 测试行；正常接线及主机测试基线均为 0。
接线检查通过 `PYTEST_ADDOPTS` 只选择新增测试，主机行没有测试名过滤。

| 临时修改 | 实际门禁行 | 退出码 | 已核对的失败原因 |
| --- | --- | ---: | --- |
| 删除 worker 的空闲维护调用 | `pytest (gate scripts)` | 1 | 接线断言找不到 `service_fingerprint_touch()` |
| 删除确认等待中的维护调用 | `pytest (gate scripts)` | 1 | 接线断言找不到 `fingerprint_touch::service(sensor)` |
| 删除维护启动调用 | `test (host)` | 101 | 已松手触摸仍只有 1 次启动，要求为 2 |
| 同时放开高／低输入的 Ready 条件 | `test (host)` | 101 | `touch before ordinary sleep must be refused`，休眠前触摸被错误接受 |
| 业务取消／到期时终止后台启动 | `test (host)` | 101 | `request cancellation or expiry must not disable background maintenance`，请求错误地禁用了维护 |

原始结果（本地文件：`target/touch-auto-rearm-validation-2026-10-23/mutations.json`）和
执行脚本（本地文件：`target/touch-auto-rearm-validation-2026-10-23/mutation-runner.py`）位于忽略的
`target/`，清理构建目录前应另行保存。故障修改未写入本工作区源码。

## 原定实机验收项目

软件验证结束时本候选未部署；后续实机失败见本文开头所链接的记录。
原定实机项目如下，未完成的项目不记作通过：

1. 连续多轮 TOUCHOUT 确认，期间完整松手，始终不按 BOOTSEL。
2. 空闲触摸松手后再发起请求；旧输入不能提前确认，新触摸仍能成功。
3. 窗口打开前长按、维护期间触摸，以及取消／超时后重新请求。
4. GP2 持续高、持续低及模块故障时，BOOTSEL、取消与期限仍正常。
5. 既有 PIN／模板／凭据保留，以及登记／匹配与维护切换的实际行为。

这些软件结果不替代物理 UP／UV、重复触摸、长期静置或不同供电条件的验收。
