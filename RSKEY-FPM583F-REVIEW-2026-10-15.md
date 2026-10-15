<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# FPM583F 分阶段代码审查记录

2026-10-15。静态审查和定向缺陷复现确认四处行为问题、一处持续门禁遗漏，另有一项实物语义待确认。用户随后要求暂时搁置实物项，继续补查软件；新增 R5，补跑 workspace、受影响配置及生物矩阵。E1 再次出现，确认外层收到 SIGTERM，发送方待查。审查阶段仅记录和离线验证，没有修复实现或操作设备。

后续：用户先授权修 R2，再授权修 R1。两项均已完成修复和离线验收，原始完整门禁退出 0；R3、R4、R5 及 E1 保持原状态，实物项继续搁置。

## 范围与判定

以总计划和工程实施计划为契约，检查阶段 1–5 及追加供电/触摸实现的完整业务链、跨阶段衔接、错误传播、授权隔离、取消、持久恢复和测试差异。阶段 6–7 已明确保留的实物及客户端验收，单独列作未验证边界。

起点为分支 `fpm583f-dev`，HEAD `c7c948cd`，审查开始时工作区干净。历史验收记录不替代本次验证；静态发现、离线复现和硬件证据分别标注。

## 本轮覆盖

| 阶段 | 已检查的链路 | 结论索引 |
| --- | --- | --- |
| 1：基线与资源 | 根/emu/flake 锁定身份、2MB 分区、真实目标 Clippy/链接、GPIO 所有权及运行时 LED 覆写 | 本轮资源与布局符合已记录预算；持续门禁遗漏见 R4 |
| 2：标准客户端基线 | TCP 报告边界、超时/EOF、进程退出错误、普通重启的 Fs 扫描及文件写入错误向上传递 | 24 项客户端通过；未再发现本阶段独立缺陷 |
| 3：UP/UV 与执行桥 | HID/CCID 归属、续取/枚举/token 权限和 RP 范围、独立 PIN/UV 重试、取消和旧响应排空 | R1、R2；42 项原指纹客户端及共享主机测试通过 |
| 4：标准管理与恢复 | PIN/BE 认证前置、pending→saved→authorized、删除前停用、断点与未知结果、Reset 标记、全部清理入口及 seed 初始化 | R2、R5、H1；原有 host 故障用例通过，矩阵 pytest 通过但外层异常见 E1 |
| 5：真实协议适配 | 手册原页、双校验/长度/命令/槽号、有界 I/O、六有效样本、保存/删除结果及取消终态 | R3、H1；矩阵 pytest 通过但外层异常见 E1 |
| 追加供电与触摸 | 请求/会话持有、启动与休眠失败、UART 停用/恢复、触摸代次/单次消费、编译期和运行时引脚排他 | 生产模型 host 用例及候选目标构建通过；真实电气/时序仍属阶段 6 |

采样数按用户已确认的六个有效样本及最终保存/目录提交判定；没有把该既定策略与手册 PROC 百分比的差异重新当作缺陷。模块/总线可信、数据库不被外部改写是既定假设，本轮也未把缺乏 UART 响应密码学认证重新列为实现 bug。

## 建议修复顺序

建议按 `R2 → R1 → R5 → R3 → E1 → R4` 推进。先恢复独立 PIN 备用通道，再修复取消恢复和残留隔离保护，随后处理采样反馈、运行器异常及持续门禁。R1、R2 已完成修复，其余项待后续安排。

| 顺序 | 事项 | 修复目标与验收重点 | 状态 |
| --- | --- | --- | --- |
| 1 | R2：PIN/UV 重试分离 | 正确 PIN 恢复自身预算；异常 UV 记录保持拒绝，实际存储错误明确失败 | 已修复，完整门禁退出 0 |
| 2 | R1：取消状态残留 | 明确请求归属与取消位生命周期；保留当前请求取消及旧响应排空，后续新请求能够恢复。回归覆盖真实 Arbiter 与固件 Board 的接线契约 | 已修复，完整门禁退出 0 |
| 3 | R5：空坏目录绕过隔离 | 区分记录缺失与存在但为空；坏记录继续阻止种子使用及未经模块清理的 Reset。检查共享判定的所有调用方，覆盖正常未配置和合法清理的正向控制 | 待修 |
| 4 | R3：正常采样反馈中止录入 | 将 `0x11/0x12` 按手册映射为可重试的采样反馈；保留先前有效样本，失败样本不计数。用合法字节应答覆盖继续采样及最终保存 | 待修 |
| 5 | E1：生物矩阵外层退出 143 | 定位 SIGTERM 来源及 pytest/清理的实际退出状态；保留非零失败传播，取得完整 runner 的退出证据 | 待定位，尚未归因为项目 bug |
| 6 | R4：持续门禁遗漏 | 将实际 fingerprint / GP2 触摸 / GP3 供电目标的 Clippy、构建，以及标准、指纹、生物客户端矩阵接入适当的 CI/分层门禁，验证接线本身能够拒绝故障 | 待补齐 |

E1 是将长生物矩阵作为可靠持续门禁之前需要闭合的问题；R4 的目标编译和较短客户端检查可独立补齐。R4 完成前，每项固件修复仍需手动验证实际指纹候选配置及受影响客户端，并运行原始完整门禁。

H1 及阶段 6–7 的实物验证继续暂缓。后续实施若遇到新的意外情况，先暂停反馈并确认处理方式。

## 问题

### R1 / P1：固件的指纹取消标志可残留到后续请求

状态：审查起点经静态调用链及离线复现确认；后续已完成授权修复和离线验收，原始完整门禁退出 0。涉及阶段 3 的执行桥，以及阶段 4–5 和供电追加实现的共用服务。以下缺陷描述和行号为审查基线。

- `Arbiter::request_cancel()` 设置 `cancel_requested=true`；`set_wait_scope()` 仅切换归属，不清除此标志。
- 固件 worker 在请求首尾只调用 `set_wait_scope()`；指纹 `Board::end()` 只撤销 processing/pending，未清除取消状态。
- 下一次 `Driver::prepare()` 先读取该标志并返回 `KeepAliveCancel`。即使旧模块操作已正常排空，新的指纹 UV 或生物管理仍可能直接失败，直到其他按钮等待路径清除标志或重新启动。
- 模拟器 `Signals::begin()` 每次请求清除 `cancel_cid`，与板级行为不同；现有模拟器取消后恢复测试不能覆盖固件的这一残留。

定位：`firmware/src/worker.rs:465/562`、`firmware/src/presence.rs:316`、`crates/rsk-device/src/fingerprint_driver.rs:617`、`tools/emu/src/signals.rs:82`。文件：[worker](firmware/src/worker.rs)、[固件 presence](firmware/src/presence.rs)、[Arbiter](crates/rsk-device/src/presence.rs)、[驱动](crates/rsk-device/src/fingerprint_driver.rs)、[模拟器 Signals](tools/emu/src/signals.rs)。

本次离线复现直接链接生产 `rsk-device/rsk-fido`，复用现有字节模块夹具，以真实 `Arbiter` 实现固件 `Board` 的时钟/processing/end 契约。先成功 prepare，匹配命令发出后请求取消，确认原任务经 `01 15` 和终态查询排空、驱动恢复 ready；模拟 worker 的 NONE→FIDO 请求切换后，下一次 prepare 实际为 `Err(KeepAliveCancel)`。只清除取消位，prepare 即恢复 `Ok(())`。这排除了模块仍忙或串口排空失败的替代原因，没有修改生产函数或夹具。

复现命令在固定 Nix 环境下退出 0（1 项观察性测试通过；通过表示残留现象已复现，不表示实现正确）。测试源和临时 manifest 副本保存在本地忽略的 `target/fpm-review-evidence/`；无实板证据。

授权后的实现由 `Arbiter::set_wait_scope(SCOPE_NONE)` 在 worker 结束 dispatch 时退役取消位：先公布空闲归属，再清除旧取消。进入活动 scope 和传感器的 processing/end 均不清除取消，因此当前请求仍可取消；逻辑请求结束也不等于模块已经排空，后续 prepare 仍须等待旧响应、取消命令及终态查询完成。未改变驱动协议、持久记录或模块写入策略。

正式回归位于 [fingerprint_presence_tests.rs](crates/rsk-device/src/fingerprint_presence_tests.rs) 和 [presence_tests.rs](crates/rsk-device/src/presence_tests.rs)。使用生产 Arbiter、驱动及字节夹具，时钟后端按固件 Board 的 processing/waiting/end 契约连接。覆盖当前取消在传感器结束后仍有效、已排空匹配后的同通道新匹配、未排空录入后的 HID/CCID 库存读取、空闲取消、跨 scope 退役及新取消保留。此测试不执行真实 IRQ；worker 四处 scope 接线、固件 Board、按钮/屏幕及模拟器调用方另经源码核对。

原始门禁在修复前的 `test (host)` 行因三处 R1 断言退出 101：新 prepare 和库存读取实际为 `KeepAliveCancel`，应当成功；旧 dispatch 的取消位应当退役，实际仍为 true。修复后的同组筛选为 31 passed，其中新增 5 项；旧请求的取消/排空及新请求的匹配/库存正向控制均通过。`bcdDevice` 由 `0x09DF` 增至 `0x09E0`。

形式模型动作和性质未改变。`HostCancelLatched` 保留比修复后固件更宽的旧取消行为，已在模型注释、形式说明及抽象登记中明确为保守过近似；现有 mutant 的历史轨迹不再当作当前 dispatch-end 清除的反例。同步行号引用及生成文档后重建引用锁；未重跑 Kani/TLC，也未扩张指纹形式覆盖。

首轮修复后完整门禁发现 `BugHostPreemptsLocalWait` 的变异补丁锚定整个旧函数，新增取消位清理使锚点失效。已缩小为仅修改公布的 scope，保留以原参数判断的 R1 清理，`expect = "killed"` 不变。临时源码副本包含当前未提交实现及测试：正常版 130 passed、退出 0；仅注入该归属缺陷后 123 passed、7 failed、退出 101。原有 `a5_a6` 明确失败为 scope NONE 下 CANCEL 实际被接受（true），应当拒绝（false），证明命中原定的跨归属缺陷，而非编译错误或仅靠 R1 新测试报红。主工作区实现未被变异。

### R2 / P1：损坏的生物重试记录会耗尽正确 PIN 的重试预算

状态：审查起点经真实 `process_cbor → clientPIN` 路径复现确认；后续已修复，定向回归和原始完整门禁均已通过，外层退出 0。涉及阶段 3 的 PIN/UV 重试分离及阶段 4 的恢复契约。

`spend_and_verify_pin_hash()` 先耐久扣减 PIN 重试；验证出正确 PIN 后，在恢复 PIN 预算之前调用 `fingerprint::restore_after_pin()`。后者先用 `retries(fs)?` 解析生物记录；若 `EF_UV_STATE` 长度、版本或补码损坏，直接返回 `Other`，使正确 PIN 的预算恢复和 token 签发都被跳过。下一次正确 PIN 又重复扣减，最终进入 `PinBlocked`。

复现使用合成 PIN、真实 ECDH/加密请求和 RAM Storage；先建立有效的 authorized 目录及合法 UV 预算，同一请求在生物记录正常时成功。仅将 `EF_UV_STATE` 置为补码错误的 `[1, 8, 0]` 后，连续八次正确 PIN 均返回 `0x7f`，PIN 剩余次数依次为 `7…0`；第九次为 `PinBlocked`。未发送匹配、未使用错误 PIN、未故障注入 PIN 存储。这不是单纯拒绝不可信 UV 状态，而是让独立备用验证方式损失预算和可用性。

审查基线定位：[PIN 验证](crates/rsk-fido/src/clientpin.rs)的 `spend_and_verify_pin_hash`（809、883–885 行）、[生物重试恢复](crates/rsk-fido/src/fingerprint.rs)的 `restore_after_pin`（261–265 行）。原有损坏重试记录测试只证明 UV 被拒绝，没有覆盖正确 PIN 的完整恢复路径。

授权后的修复先耐久恢复 PIN 预算，再尝试恢复有效的 UV 预算。内部读记录区分 Missing/Valid/Invalid 与实际存储错误；缺失、坏格式和未知版本保持原记录及 UV 拒绝，允许正确 PIN 取得正常权限的 token 或修改 PIN。实际 UV 读取、写入及恢复回读失败仍传播错误，不签发新 token。记录格式、模板、PIN/passkey 派生保持不变。

正式回归在 [clientpin_fingerprint_tests.rs](crates/rsk-fido/src/clientpin_fingerprint_tests.rs)，沿用现有 ClientPIN/ECDH 客户端夹具并走真实 CBOR 分派。修复前的原始 `check.sh` 在 `test (host)` 行退出 101，失败明确为正确 PIN 应成功却返回 `0x7f`；不是格式、编译或环境故障。修复后的 5 项回归覆盖两种 PIN 协议、旧版及权限式 token、连续正确 PIN、坏记录保持和重开、错误 PIN、修改 PIN/种子连续性、两处 UV 读取故障及 PIN/UV 写入失败。FIDO 全套为 742 passed、1 项原有 ignored；原始完整门禁最终显示 `ALL CHECKS PASSED` 并退出 0，指纹候选目标 Clippy/release 构建也已退出 0。

源码中的 PIN 预算恢复移位影响 5 处形式说明引用，已按实际写入和 mismatch 清零位置更新并重建引用锁；模型动作、性质和指纹形式 gap 未改变。新增内部读取函数使泛型函数、总函数及 Storage 约束计数各增加 1，已同步 `docs/verified-compilation.md`，并将对应自测的错误计数注入从 `211 → 212` 同步为 `212 → 213`，仍明确断言错误值被拒绝。`bcdDevice` 从 `0x09DE` 增至 `0x09DF`，同步协议、FIDO 指南及 CHANGELOG。修复前后日志分别为 `r2-gate-before.log`、`r2-regression-before.log` 和 `r2-regression-final.log`，位于本地 `target/fpm-review-evidence/`。

### R3 / P2：两类正常采样反馈被当作通信故障，中止整次录入

状态：手册原页核对及生产驱动离线复现确认。涉及阶段 5 的模块结果映射与阶段 4 的录入会话衔接。

通用手册 V1.2.0 印刷页 17–18、图 11 将 `0x11` 和 `0x12` 定义为移动范围过大/过小，建议重试当前样本。`Driver::module_error()` 未映射这两个完成码，落入 `Failure::Communication`，而 `fail()` 清除临时录入上下文；`bio_management::sample()` 收到错误后也清除会话。这违背已经确定的“失败可重试当前样本”规则：一次普通移动反馈会丢失前面已经采集的样本流程，客户端只得到 `Other`。

离线复现先完成一个好样本，再分别输入 `0x0e`、`0x11`、`0x12` 的合法模块应答：`0x0e` 控制组返回 `PoorQuality/remaining=5`，可以继续；另两组实际均返回 `Err(Other)`。模块仍健康、保留 1 个好样本，但后续相同槽位/代次/remaining 的继续采样仍返回 `Other`，且没有发送新采样命令。现有字节集成夹具只覆盖 poor/nofinger，未覆盖这两个手册结果。

定位：[模块错误映射](crates/rsk-device/src/fingerprint_driver.rs)的 `module_error`（434–464 行）/`fail`（172 行）、[录入会话](crates/rsk-fido/src/bio_management.rs)的 `sample`（150–152 行）。未修改映射或恢复逻辑。

### R4 / P2：指纹目标及新增标准客户端矩阵未接入持续门禁

状态：构建/CI 调用链静态确认；这是验证遗漏，不是本次目标构建失败。

`scripts/check.sh` 显式编译 default、display、fips 等固件配置，但没有任何 `--features fingerprint` 或 GP2/GP3 候选配置的编译行；其默认 emu host test 也不启用 `fingerprint-fixture/bio-fixture-2mb`。CI 的 flavors/knob/emu 配置没有补上这些分支，因而 `firmware/src/fingerprint.rs`、`fingerprint_touch.rs` 和实际引脚排他断言不在指纹构建检查中。

`standard-client.sh/fingerprint-client.sh/bio-client.sh` 只登记在 `reproduce.sh` 的手动 emu phase，现有 CI 仍仅调用普通 `emu-suites.sh`，没有自动执行新增标准客户端矩阵。普通 host 行覆盖共享纯逻辑，不能替代真实板级桥的编译或客户端调用链。

本次手动运行候选配置的 Clippy/release 链接已通过，阶段二、三客户端也已通过；这些结果不消除持续门禁的遗漏。定位：[check.sh](scripts/check.sh)、[CI](.github/workflows/ci.yml)、[模拟器 CI](.github/workflows/emulator.yml)、[手动复现登记](scripts/reproduce.sh)、[普通 emu suites](scripts/emu-suites.sh)。审查阶段没有调整或重跑完整门禁；后续 R2 的完整门禁执行另记，原始 `check.sh` 和 CI 接线仍未修改。

### R5 / P2：空的坏目录记录被当作未配置，绕过停用指纹构建的残留保护

状态：补查软件边界后，由真实 `process_cbor → Reset` 及 `Ctx::load_keydev()` 路径离线复现确认。涉及阶段 4 的坏记录处理和切换构建后的隔离。

`configuration_clean()` 使用 `try_has_data()` 判断目录和孤立模板是否存在；该函数对零长度记录也返回 false。头部为空且没有非空模板记录时，函数提前返回 `Ok(true)`，跳过原本会拒绝该记录的 `read()`。零长度目录确实仍是一个存在但格式错误的记录，不能据此证明从未配置或已完成生物清理。

复现先通过生产 `reconcile()` 对未知库存建立合法 isolated 目录。在没有指纹服务的上下文中，有效隔离记录令 Reset 返回 `NotAllowed`，种子读取也拒绝；非空坏记录同样被拒绝。只把目录记录换成空值后，`try_read` 明确为 `Some(0)`、目录解析仍报错，但 `configuration_clean()` 返回 `Ok(true)`，种子读取成功，真实 CTAP Reset 返回 `0x00` 并删除目录。该路径没有模块服务，未执行模块清库。

这不是正常录入已证明会产生空记录，而是损坏记录处理存在缺口；当前证明限 RAM Storage 和软件入口，不推断实物故障发生概率。定位：[目录清洁判定](crates/rsk-fido/src/bio_directory.rs)245–256 行、[Reset 前置保护](crates/rsk-fido/src/bio_management.rs)287–292 行、[种子使用保护](crates/rsk-fido/src/lib.rs)110–120 行。共享判定也用于 Management/屏幕擦除及种子初始化；本次实际复现覆盖种子使用和无指纹 CTAP Reset，没有宣称逐一运行所有入口。

## 待确认的恢复语义

### H1：删除本来不存在的模板能否收尾

这是有代码依据、但尚未取得实物结论的问题，不计入已复现的 R1–R3。手册 p45 明确提到删除不存在的 ID 可能报错，p25 的具体删除结果表却未给出该错误码。现有脚本和字节夹具均把删除空槽当作成功。

取消/重启后的 pending 可能尚未在模块保存，或删除已经成功但主控未能写入 retired。`bio_management::delete_entry()` 在前置库存已可确认空槽时仍发 Delete；驱动只接受 OK，收到任何非忙错误都会中止，不进入库存确认。若实物对空槽报错，经 PIN 授权的取消/删除可能永久停在 deleting，槽位不能正常回收，与第五阶段“成功、模板本来不存在、结果未知分别表达”的计划存在缺口。

需在阶段 6 核对空槽删除的真实 ACK/结果码，并验证“未保存的 pending 取消”和“删成功但主控提交前重启”两条恢复链；当前不把未知码硬映射为成功，也未重发或修改删除策略。

## 执行意外：E1 / 外层运行器重复退出 143，信号来源待查

`scripts/bio-client.sh` 的完整日志最终为 `69 passed, 2 skipped in 1006.83s`；但随后读取该工具会话最终状态，实际得到 `exit_code=143`。测试断言通过与外层命令成功是两个不同结论，当前只能确认前者。尚未判断是脚本/清理、信号传递还是执行环境导致，不能把它归为已证实的项目 bug，也不能忽略退出码。

此前根据 pytest 汇总给出的“矩阵结束/通过”只适用于 pytest 层，记录中“runner 退出 0”的表述已撤回。初次发现后按用户要求暂停；用户要求继续非实物范围后，在不改脚本的前提下，以 `bash -x scripts/bio-client.sh` 完整复跑，并在外层 shell 记录 TERM 和退出状态。

复跑汇总 `69 passed, 2 skipped in 1000.72s`，工具会话最终仍退出 143。外层记录只有 `wrapper_signal=TERM`，没有执行到原定的 `outer_status`/完成时间写入；最后的脚本跟踪为 EXIT trap 的临时目录清理。由此确认外层确实收到 SIGTERM，但尚未取得发送方、pytest 本身的最终进程状态，或足以归因到脚本/执行环境的证据。不能把它算成已确认的项目实现 bug，也不能把整条 runner 当作退出 0。

初次原件为 `target/fpm-review-evidence/bio-clients.log`、`bio-runner-status.txt`；复跑原件为 `bio-clients-followup.log`、`bio-runner-followup.status`。没有修改业务代码、测试或 runner，也没有通过忽略非零状态继续认定成功。更深的信号来源定位尚未进行。

## 审查阶段验证

- 固定 Nix / Rust 1.96.0，`cargo test --frozen -p rsk-device -p rsk-fido -p rsk-usb --target x86_64-unknown-linux-gnu` 退出 0：分别 125、737、63 项通过；FIDO 原有 1 项 ignored 保留。说明现有测试全绿仍未捕获 R1/R2，不代表这些缺陷不存在。
- 初轮三项离线观察性复现通过，实际打印残留取消、正确 PIN 递减及采样反馈中止现象；补查加入空目录 R5 后，按保存的可移植命令重跑，四项均通过并退出 0。通过表示缺陷现象复现，不表示实现正确。[可复现源码和命令](RSKEY-FPM583F-REVIEW-EVIDENCE/README.md)随记录保留，最终日志为 `target/fpm-review-evidence/repro-followup.log`。
- 默认沙箱首次启动 Nix 时因缓存锁文件只读而失败，测试没有启动；申请执行权限后，相同离线环境正常运行。没有以环境错误计作业务复现。
- 标准生物管理 / FPM 字节矩阵的 pytest 汇总为 `69 passed, 2 skipped in 1006.83s`；两项 skip 仅为常供电组合不适用的开关供电故障用例，与既有矩阵设计一致。外层运行器实际退出 143，整条命令未判定成功，见 E1。
- 无屏 2MB / GP2 触摸 / GP3 供电配置的实际目标 Clippy（`-D warnings`）和 release 链接已退出 0；未生成分区表、未部署。资源核对见下节。
- 阶段三原客户端 `42 passed in 38.06s`，阶段二原客户端 `24 passed in 7.92s`；各 runner 退出 0。
- 补查执行全部可主机测试的 workspace（明确排除 `firmware/rsk-wipe` 二进制），2366 项通过、5 项原有 ignored（含 doc-tests），退出 0；日志 `host-workspace-followup.log`。另对 Device 的 display/strict-config、FIDO 的 fips-profile/strong-pin/largeblob-ext 及 emu 的 bio-fixture-2mb 单独回归，分别 129/116/731/741/733/122 项通过，合计 2572 项，整组退出 0；三个 FIDO 配置各保留 1 项原有 ignored。日志 `host-features-followup.log`。
- 生物矩阵完整复跑仍为 69 passed、2 skipped，但外层收到 TERM 并退出 143，未判为完整 runner 成功，见 E1。
- 已按保存的可移植说明原样重跑复现，3 项通过；补全 R2 已授权目录正向控制后再次退出 0。新记录的 SPDX/版本守卫、复现源码 rustfmt 与本地链接检查均退出 0；最终链接检查结果在日志中保留。
- 补查记录和四项复现源码的 SPDX、版本守卫、rustfmt 及本地链接检查全部退出 0，20 条链接均有效；检查日志为 `followup-record-checks.log`。确认实现和三个锁文件仍无修改。
- Cargo.lock / flake.lock / emu Cargo.lock 的 SHA256 仍分别为 `e0719d516ac008462724a853d57b5c0fb25425d1de72e2a19966c9a7f0912481` / `329b2d37efcae49a4dec8a14d28a8257304d999f78f6cfdd4e219b7d944b281b` / `7fb1e8eba983b8eaeb8a509238fb9498f081725b9a57e4bf9748e05230f7adf0`。

## R2 授权修复的离线验证

以下命令均在固定 Nix 环境中执行。原始门禁使用 `HOST_TARGET=x86_64-unknown-linux-gnu CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 ./scripts/check.sh`，没有删除、跳过或放宽检查。

| 验证 | 实际结果 | 本地日志（`target/fpm-review-evidence/`） |
| --- | --- | --- |
| 修复前，新回归接入原始门禁 | `test (host)` 因正确 PIN 返回 `0x7f` 而退出 101 | `r2-gate-before.log` |
| 修复后，5 项正式回归 | 5 passed，退出 0 | `r2-regression-final.log` |
| 修复后，FIDO 默认全套 | 742 passed、1 项原有 ignored，测试通过；随后的引用检查发现需同步的 5 处引用 | `r2-fido-and-citations.log` |
| 形式引用、边界表及版本检查 | 更新引用后整组退出 0，未改变模型或性质 | `r2-citation-refresh.log` |
| 指纹候选目标 Clippy / release 链接 | 两步均通过，整组退出 0 | `r2-fingerprint-build.log` |
| 文档构建及离线链接 | 0 errors，退出 0 | `r2-docs-check.log` |
| 现有指纹 / 标准客户端 | 42 / 24 passed，两个脚本所在整组退出 0 | `r2-fingerprint-clients.log`、`r2-standard-clients.log` |
| 四组独立审查证据 | 4 passed，退出 0；R2 验证正确行为，R1/R3/R5 仍复现缺陷 | `r2-review-evidence.log` |
| 首轮修复后完整门禁 | 固件及主机矩阵通过，在 11C 统计文档检查因新增函数的 3 个计数未同步而退出 1 | `r2-gate-after.log` |
| 同步统计文档后定向核对 | 24 项测量通过，退出 0 | `r2-function-count-refresh.log` |
| 同步统计文档后的完整门禁 | 3713 项门禁自测通过，1 项因旧计数替换未注入错误而失败，退出 1 | `r2-gate-final.log` |
| 同步统计自测夹具后定向核对 | 14 passed，退出 0；仍验证错误计数 `213` 被实际计数 `212` 拒绝 | `r2-function-count-fixture.log` |
| 最终原始完整门禁 | `ALL CHECKS PASSED`，外层退出 0；门禁自测 3714 passed、CLI 312 passed、interop 45 passed | `r2-gate-complete.log` |

候选资源和三个锁文件的核对命令退出 0，见 `r2-resources-and-locks.log`。审查记录及证据说明的 22 条本地链接有效，检查退出 0，见 `r2-record-links.log`。上述客户端回归未运行 E1 所涉及的长生物矩阵，因此不改变 E1 的未闭合状态。

完整门禁最后留在默认 `target/thumbv8m.main-none-eabihf/release/firmware` 的是 no-touch 测试镜像，不作为部署产物。独立的指纹候选构建位于 `target/fpm-review-build/`，本轮没有刷机或实物验收。

## R1 授权修复的离线验证

命令沿用固定 Nix 环境、Linux host target、离线 Cargo 和两路构建。完整门禁仍为原始 `./scripts/check.sh`；没有增加、删除或放宽门禁行，R4 仍未修。

| 验证 | 实际结果 | 本地日志（`target/fpm-review-evidence/`） |
| --- | --- | --- |
| 修复前，新回归接入原始门禁 | `test (host)` 三处 R1 断言失败，退出 101 | `r1-gate-before.log` |
| 修复后，presence 筛选 | 31 passed，含新增 5 项，退出 0 | `r1-regression-after.log` |
| 最终通道接线契约及主机 Clippy | HID CID / CCID CHANNEL_NONE 三项驱动回归通过，Clippy `-D warnings` 退出 0 | `r1-presence-contract.log`、`r1-presence-clippy.log` |
| Device / FIDO / USB 主机测试 | 130 / 742 / 63 passed，FIDO 保留 1 项原有 ignored；随后引用审计指出待同步引用，整组退出 1 | `r1-host-and-citations.log` |
| 引用、生成文档、版本、SPDX、格式及文档构建/链接 | 完成引用内容核对后整组退出 0；文档链接 0 errors | `r1-docs-and-citations.log` |
| 指纹候选 Clippy / release 链接及资源/锁文件核对 | 整组退出 0，无设备操作 | `r1-fingerprint-build.log` |
| 四组独立审查证据 | 4 passed，退出 0；R1/R2 验证正确行为，R3/R5 仍复现缺陷 | `r1-review-evidence.log` |
| 原有指纹 / 标准客户端及取消子集 | 42 / 24 / 10 passed，整组退出 0；取消子集另有 61 deselected | `r1-fingerprint-clients.log`、`r1-standard-clients.log`、`r1-bio-cancel-clients.log` |
| 首轮修复后完整门禁 | 在 `comutants lint` 因旧函数锚点失效退出 1；此前固件、主机及模拟器矩阵通过 | `r1-gate-after.log` |
| 同步变异补丁后的定向验证 | lint 退出 0；正常副本 130 passed、退出 0；保留 R1 清理的归属 mutant 被原有断言拒绝、退出 101；验证器整组退出 0 | `r1-comutant-check.log`、`r1-comutant-control.log`、`r1-comutant-mutated.log` |
| 变异工具自测 | 80 passed，退出 0 | `r1-comutant-tests.log` |
| 最终原始完整门禁 | `ALL CHECKS PASSED`，运行器实际退出 0；门禁自测 3714 passed、CLI 312 passed、interop 45 passed | `r1-gate-final.log` |

筛选的生物客户端命令为 `bash scripts/bio-client.sh -k "cancel or draining or session_end"`。这不是 E1 涉及的完整长矩阵，也不用于闭合 E1。回归没有运行硬件或重新验证物理 IRQ 时序。

记录与证据说明的 26 条本地链接有效，版本、SPDX、引用及差异空白检查通过；三个锁文件与审查基线一致。另核对 R2 原有七处跟踪文件差异保持不变，安全模型去除注释/空白后的内容未改变。日志为 `r1-record-checks.log`、`r1-scope-check.log`；收尾核对为 `r1-final-checks.log`。

## 候选镜像资源

`FLASH_SIZE=2097152 KVMAIN=896K FINGERPRINT_POWER_PIN=3 FINGERPRINT_TOUCH_PIN=2`，`--features fingerprint`，无 display/no-touch。审查基线代码为 846996B；R2 修复后为 847100B，增加 104B；R1 修复后为 847124B，再增加 24B。静态 RAM 仍为 `__ebss - __sdata = 344364B`，Core0 栈区域仍为 179904B，比 168KiB 下限多 7872B；这不代表已测量运行时栈峰值。

ELF 符号给出 code/main/counter 边界 `0..0x100000` / `0x100000..0x1e0000` / `0x1e0000..0x200000`。产物位于 `target/fpm-review-build/`，只有用于审查的 raw ELF，不含分区表；构建和符号检查未运行 ELF，也未覆盖原 gate 的产物。

## 未验证边界及保持不变的内容

审查阶段未修改实现；后续 R1、R2 已完成修复和离线验收，R3、R4、R5 未修。H1 及阶段 6–7 的实物项按用户后续要求暂时搁置。E1 已确认重复 TERM，信号来源尚未闭合。新一轮 Kani/长期 fuzz 及真实硬件均未重跑；本轮不是全项目无缺陷证明，也不是部署验收。阶段 6–7 的触摸/供电可行性、UART/IRQ/XIP/栈实测、真实 USB/浏览器/第二管理客户端仍按原计划未验收。

审查阶段仅新增此记录和复现证据；后续 R2 修复涉及上述实现、回归、版本、相应说明/引用，以及统计自测夹具同步。R1 涉及取消位生命周期、正式回归、版本及说明/引用，并同步保留原缺陷语义的变异补丁锚点。依赖、原始 `check.sh`、CI 接线及既有阶段验收记录保持不变，没有提交、推送或设备动作。
