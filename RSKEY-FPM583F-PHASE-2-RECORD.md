<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# 第二阶段执行记录

2026-10-11。2.1–2.8 验收通过：固定标准客户端 24 项测试通过，仓库原始完整 gate 退出 0，输出 `ALL CHECKS PASSED`。对应[第二阶段计划](RSKEY-FPM583F-PHASE-2-PLAN.md)，原始证据及 gate 摘录见[验收证据](RSKEY-FPM583F-PHASE-2-EVIDENCE/README.md)。

## 1. 固定环境与目标（2.1–2.3）

- 开始时 HEAD：`08f6ea71cfde63edf0d95c8775606b6e972e53b9`，分支 `fpm583f-dev`，工作树干净；业务来源为 `7c9a57836647c363a9f006da1892e886869afb61`。本阶段没有提交、推送、设备操作或生产签名。
- 锁定 Nix 环境：Python 3.13.13、Rust/Cargo 1.96.0，主机目标 `x86_64-unknown-linux-gnu`。三个依赖锁文件保持[第一阶段](RSKEY-FPM583F-PHASE-1-RECORD.md)记录的校验值。
- [env.nix](tests/standard_client/env.nix)使用既有锁定 nixpkgs，隔离安装 python-fido2 **2.2.1**、cryptography 48.0.0、pytest 9.0.3、cffi 2.0.0、pycparser 3.0。官方 fido2 wheel SHA256 为 `ed397da981b9ab133da6ead7309e41f924b566b749956129efe286fae097749f`。入口清除 `PYTHONPATH/PYTHONHOME`，防止原 shell 的 fido2 2.1.1 覆盖隔离解释器；依赖仅用于测试。
- 原模拟器 SHA256：`7bc5f6cf60ba29f2da86a5985450df36083a4d689624ebea739a8ad41fa832c8`；修复后 debug 产物：`d1b9e5d38fef52173631f0a10648f1d650f3f852afff75be4926c108e08aa47c`。
- 每个集成用例拥有独立进程、回环端口和合成存储，参数为 `--host 127.0.0.1 --fido-port <端口> --ccid-port 0 --touch --store <绝对路径>`；使用既有 `/dev/urandom`，不枚举或回退到硬件。

传输、截止时间、确认输入和日志边界统一说明于[客户端 README](tests/standard_client/README.md)。桥仅提供 TCP 的 64 字节报告通路；CTAPHID、CTAP2、ClientPin、WebAuthn 及 RP 验签均由固定版本标准库执行。自动终端回复属于合成测试输入，不作为真人或物理存在证据。

## 2. 完成的闭环（2.4–2.7）

主流程以 required PIN UV 注册 discoverable credential，无 allowList 认证，通过原注册公钥和 userHandle 的 RP 校验。操作结束后主动停止进程，挂载同一文件、重建连接和 PIN token，用新 challenge 认证旧凭据；没有重新注册或设置 PIN。签名计数器从 1 增至 2，存储为 1572880 字节，该观察仅证明正常重启持久性。

独立 allowList 流程观察到合法 UP-only 断言（UP=true、UV=false）。负向测试检查具体原因：拒绝为 `OPERATION_DENIED`；观察 `UPNEEDED` 后取消为 `KEEPALIVE_CANCEL`，同一连接可继续新操作；错误/缺失 PIN、跨 RP 凭据、错误 origin、旧 challenge、篡改签名和缺失 required UV 均被对应层拒绝。传输 EOF、超时和断开保持原错误并关闭连接；不兼容存储拒绝启动且原字节保留。具体断言见[认证测试](tests/standard_client/test_ceremony.py)、[失败测试](tests/standard_client/test_failures.py)与[传输测试](tests/standard_client/test_transport.py)。

## 3. 发现与修复

原模拟器首次认证成功，但正常重启后旧驻留凭据返回 `NO_CREDENTIALS`；[修复前日志](RSKEY-FPM583F-PHASE-2-EVIDENCE/restart-before-fix.log)退出 1，失败发生于真实 CTAP getAssertion。根因是启动时遗漏固件已有的 `Fs::scan()`，导致 present/dynamic 索引未恢复。修复仅在[模拟器 boot block](tools/emu/src/device.rs)增加 `fsb.scan()`，已检查冷启动、软件 Replug、warm reboot 及普通/display 模式共用此入口。持久格式与固件未改，`bcdDevice=0x09DA`，CLI/TUI 版本保持。

前两次完整 gate 分别指出新 runner 未登记和新增一行导致源码引用漂移，具体错误见[gate 摘录](RSKEY-FPM583F-PHASE-2-EVIDENCE/README.md#gate-results)。随后将入口登记并接入 `reproduce.sh` 的 `emu` phase，按实际代码同步四条引用锁记录；锁定内容、模型语义与 trace 规则保持。原始 gate 重新从头运行通过。

回顾发现 P2：`Emulator.stop()` 忽略提前退出。先通过实际 runner 复现 **3 failed、21 passed，退出 1**，三项均因应拒绝的退出未抛错而失败；见[修复前日志](RSKEY-FPM583F-PHASE-2-EVIDENCE/exit-before-fix.log)。修复仅接受自身对存活进程发送 SIGTERM 后的正常结束，并在断言前关闭资源、清除进程引用。[修复后日志](RSKEY-FPM583F-PHASE-2-EVIDENCE/exit-after-fix.log)为 **24 passed，退出 0**。

| 退出情形 | 修复前 | 修复后 |
| --- | --- | --- |
| 提前 SIGKILL，`-9` | 被接受，回归因未抛错报红 | 拒绝，资源关闭 |
| 提前 SIGTERM，`-15` | 被接受，回归因未抛错报红 | 拒绝，不能冒充本次主动停止 |
| 提前退出 `0`，未声明预期 | 被接受，回归因未抛错报红 | 拒绝 |
| 清理向存活进程发送 SIGTERM | 正常重启通过 | 正常重启继续通过 |
| 已确认不兼容存储拒绝且退出 `0` | 原负向用例通过 | 检查原因、原字节及退出码后显式接受 |

## 4. 验收与复用（2.8）

```sh
export NIX_CONFIG='experimental-features = nix-command flakes'
nix develop --no-update-lock-file -c bash scripts/standard-client.sh
nix develop --no-update-lock-file -c env HOST_TARGET=x86_64-unknown-linux-gnu \
  CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true ./scripts/check.sh
```

| 检查 | 实测结果与证据 |
| --- | --- |
| 标准客户端最终测试 | 24 passed，0 failed/skipped，退出 0；[原始日志](RSKEY-FPM583F-PHASE-2-EVIDENCE/exit-after-fix.log) |
| 模拟器既有 host tests | 118 passed，0 failed/ignored/filtered；[原始日志](RSKEY-FPM583F-PHASE-2-EVIDENCE/emu-host-tests.log) |
| 复现入口登记 | 124 gate rows、21 runners、19 jobs、14 phases、8 refusals；mutation tests 24 passed；[原始日志](RSKEY-FPM583F-PHASE-2-EVIDENCE/runner-registration-check.log) |
| 引用同步 | citation gate 通过，引用/trace 测试 174 passed；[原始日志](RSKEY-FPM583F-PHASE-2-EVIDENCE/citation-sync-check.log) |
| 原始完整 gate（退出检查修复后） | 退出 0，`ALL CHECKS PASSED`；脚本 3713 passed（664.76 秒）、CLI 312 passed、interop 45 passed、gitleaks 无泄漏；[摘录与原件校验值](RSKEY-FPM583F-PHASE-2-EVIDENCE/README.md#final-gate)。既有 Rust ignored 项按原 gate 保留 |
| 文档与守卫（退出检查修复后） | 文档构建、729 个离线链接（0 Errors）、citation/claims/run-count/SPDX/bcd 均通过；[原始日志](RSKEY-FPM583F-PHASE-2-EVIDENCE/exit-fix-docs-check.log) |
| shell 语法与 diff 空白 | `bash -n`、`git diff --check` 退出 0 |

完整重复日志已移入 Git 忽略的本地 `RSKEY-FPM583F-PHASE-2-EVIDENCE/raw/`；提交范围保留关键小型日志、逐行摘录及原件 SHA256，详见[归档说明](RSKEY-FPM583F-PHASE-2-EVIDENCE/README.md#local-archive)。合成 `.store` 由 runner 清理，不作为交付物。

## 5. 有效范围与后续

本阶段验证共享 FIDO 处理链、标准库客户端、RP 验签及普通文件持久性。模拟器 `--touch` 声明 `shows_confirm=true`，会询问 PIN token 权限确认；无屏板行为另验。当前 UV 来自 PIN，没有内建指纹 UV；模拟器 flash 几何为 352/32 sector，目标 2MB 板为 224/32。

实物 USB、浏览器/多系统互通、指纹、触碰新鲜性、2MB 容量、运行时栈、IRQ、掉电耐久、OTP 和安全启动仍待验证。第二套客户端与 USB/IP 按后续计划补充；步骤 3 可复用当前连接、Interaction 与 RP 夹具，生物结果从模块服务进入。完整 gate 留下 no-touch 测试镜像，本阶段没有部署到设备。
