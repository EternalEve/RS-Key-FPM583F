<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# RS-Key 宏观审查：问题、建议方案与处理顺序

2026-10-16。范围：项目架构、安全默认值、存储错误语义、构建产物与开发规范。

本记录来自简要静态阅读，不是完整安全审计，也没有确认新的可利用漏洞。
审阅时分支为 `fpm583f-dev`，HEAD 为 `90580173`；工作区已有未提交改动，
因此结论针对当时的实际工作树，不等同于该提交或已发布固件的结论。
原始审查只新增本文，当时以下实现建议均未执行；后续 M4 实施记录见第 7 节，
2026-10-17 的 M5 规范纠偏、理由和验证边界见第 8 节；M2 的授权与兼容性
评估、模拟器恢复回归和已选定的严格日用发行策略见第 11 节。

## 1. 总体判断与范围

项目的硬件适配、协议 applet、存储及可主机测试逻辑分层较清楚，已有较多
主机测试、模拟器测试和门禁。主要风险在于部分安全保证依赖人工调用约定、
默认发行策略及尚未完成的硬件验证，不能仅凭测试数量或门禁通过推导整机安全。

下文区分当前可直接核对的工程问题、已记录的设计取舍和待验证假设。
既有指纹审查中的 R1、R2、R3、R4、R5 已记录为完成修复或补齐并通过离线验收，
不在这里重新列为未修复问题；R4 的远端 CI 尚未触发。
E1 的历史信号来源及 H1 的实物语义仍以
[专项审查记录](RSKEY-FPM583F-REVIEW-2026-10-15.md)为准，本文不恢复其调查或设备操作。

## 2. 问题总表

| 编号 | 问题 | 当前证据性质 | 建议优先级 | 处理位置 |
| --- | --- | --- | --- | --- |
| M1 | 关键硬件保证缺少完整实测证据 | 已登记的待验证假设；不是已确认硬件故障 | 交付前必须核实 | 独立硬件工作线 |
| M2 | 默认管理配置允许部分无确认写入 | 严格日用包、显式兼容包及模拟器恢复门槛已落地并通过完整离线门禁；旧证据包仅同步配置摘要，实物兼容性独立待验 | 高，涉及日用安全策略 | 软件第 4 步，已完成离线验收，见第 11 节 |
| M3 | 存储 API 容易混淆缺失与读取失败 | 功能修复与契约迁移通过记录修正后的当前工作区完整门禁；既有 Kani state 安全回归未完成，资源问题独立跟踪 | 高，影响跨 applet 的异常处理 | 软件第 3 步，功能进展见第 9 节，资源后续见第 10 节 |
| M4 | 测试固件覆盖正式固件的本地输出路径 | 审查时确认的构建流程风险；已修复并完成离线验收 | 高，先降低产物误用风险 | 软件第 1 步，已完成，见第 7 节 |
| M5 | 无堆分配规范与现有 RSA、FIDO ML-DSA 堆使用不一致 | 规范纠偏已完成并通过离线验收；运行时资源实测仍待完成 | 中，修正成本较低 | 软件第 2 步，已完成规范纠偏，见第 8 节 |

实施顺序同时考虑风险、改动范围和验证成本，不代表 M5 的安全影响高于 M3。
M1 可先准备实验方案和验收表；实际设备操作遵循 [SAFETY.md](SAFETY.md)。

## 3. 问题与建议方案

### M1：关键硬件保证尚未完成验证

**问题与影响。** [架构文档](docs/architecture.md)明确记录：Flash 擦写的执行体在
SRAM，但双核暂停相关代码仍在 Flash，XIP 缓存是否足以覆盖擦写期间的取指，
属于 `PLAT-XIP-001` 的待验证条件。不能把它表述成已经发生的取指冲突。

指纹专项审查的 H1 则指出：模拟夹具把删除空槽当作成功，而实物可能报错。
如果实际返回与夹具不同，取消未保存的录入，或删除成功但主控提交前重启，
可能无法正常回收槽位。按需供电下的连续触摸也仍待实测。

**建议方案。** 使用明确授权的专用测试板，固定板型、芯片修订版、模块固件、
接线、供电方案、构建配置和镜像哈希。优先验证三组条件：

- 双核运行、Flash 擦写和 USB/IRQ 并发时的实际行为。
- 空槽删除的真实 ACK/结果码，以及未保存 pending 和删除中断的两条恢复链。
- VCC 关闭、V_TOUCH 保持供电时的连续独立触摸、持续按住和重新上电行为。

取得实测语义后再更新驱动判断和字节夹具。未知返回码、通信失败及无法确认的
库存继续保持隔离；不把所有删除错误映射为成功。具体改法由实测结果决定。

**验收标准。** 每个假设都有绑定具体硬件和镜像的结果、原始记录及失败恢复证据。
模拟器通过只证明软件模型内的行为；未测或无法复现的条件继续标为未验证。

依据：[架构](docs/architecture.md)、[平台假设](docs/platform-assumptions.md)、
[H1 记录](RSKEY-FPM583F-REVIEW-2026-10-15.md)、
[供电与触摸计划](RSKEY-FPM583F-POWER-CONTROL-PLAN.md)。

### M2：默认管理配置偏向兼容性

**问题与影响。** 默认构建允许 USB 主机无需 PIN 或触摸修改部分 DeviceInfo、
应用开关及身份配置，能够关闭应用。去重已减少相同记录的重复写入，但不断
改变配置仍可能消耗 Flash 寿命。这是已记录的设计选择，不是新发现的秘密泄露；
设备恢复出厂仍保留 presence 门槛，不能描述成无确认擦除。

**建议方案。** 优先复用已有 `strict-config`，将其作为日用发行配置的候选，
保留明确命名的宽松兼容版本。先验证常用管理工具，再决定调整默认发布包、
下载指引和推荐构建。同步列出各配置入口的授权要求和不再支持的操作。

这是产品与兼容性决策，不能仅修改说明文字后认定风险已消除。
`strict-config` 也不等于 OTP 主密钥或安全启动加固。

**验收标准。** 在所有相关传输入口验证未授权写入被拒绝、合法管理操作仍可用，
并记录兼容性变化。对选定日用配置核对持久写入路径，明确仍存在的磨损风险。

依据：[威胁模型](docs/threat-model.md)、[管理 applet](crates/rsk-mgmt/src/lib.rs)、
[现有 feature](firmware/Cargo.toml)、[发布构建](.github/workflows/release-build.yml)。

### M3：存储错误语义依赖调用者选择正确接口

**问题与影响。** `Fs::read()`、`size()`、`has_data()` 会将读取失败折叠为缺失
或 false；安全路径需要主动调用 `try_*`。底层 `Storage` 还通过独立的
`last_error()` 表达上一笔读取的失败。误用可能让故障被解释为未设置 PIN、
未配置门槛或允许重新初始化，但本轮未证明当前仍有这样的授权绕过。

**建议方案。** 分两阶段收敛，避免一次改遍所有 applet：

1. 先检查 PIN、密钥初始化、授权、备份导出、重置及恢复路径，沿读取结果追踪
   后续分支和缓存状态，使用现有 fallible 接口。不能只搜索“读取后是否写入”。
2. 再单独重构底层契约，让读取直接返回 `Result<Option<T>, E>`，逐步移除
   `last_error()` 的旁路语义；同步迁移设备、模拟器、测试及 fuzz 后端。
   允许忽略错误的展示场景应保留明确、局部的降级策略。

保留“缺失”“存在但为空或损坏”“I/O 失败”的区别。`try_has_data()` 判断的是
非空内容，不能替代需要区分 `None` 与 `Some(0)` 的存在性判断。
这项重构不应顺带改变持久格式、PIN/passkey 派生或既有升级兼容行为。

**验收标准。** 注入读取故障后，不重建默认 PIN、不覆盖已有密钥、不签发
未授权 token、不放宽权限；正常首次初始化和合法恢复仍成功。
覆盖跨命令缓存状态和存储重开，并检查每个共享接口使用者。
若发现实际缺陷，先复现再修复；纯接口重构则证明行为保持。

依据：[Fs 读取接口](crates/rsk-fs/src/fs.rs)、
[Storage 契约](crates/rsk-fs/src/storage.rs)、
[设备后端](crates/rsk-store/src/lib.rs)、[威胁模型](docs/threat-model.md)。

### M4：测试固件覆盖正式固件的本地路径

**问题与影响。** `check.sh` 的多个配置共用默认产物路径，最后留下自动确认
presence 的 `no-touch` 镜像。随后直接取该路径刷写，可能取消物理确认要求。
正式发布工作流已选择明确的发行包并拒绝带 no-touch 名称的资产，
因此这里针对的是本地产物隔离，不能据此声称已发布固件含有测试旁路。

**建议方案。** 正式、指纹候选、显示和 no-touch 构建使用可区分的输出目录，
导出的产物使用明确名称。让检查与打包入口接收目标路径或显式配置，
同步审阅尺寸、栈预算、分区表、ELF 和其他依赖默认路径的守卫。

在正式打包前核对镜像自身与构建一致的 feature 标记，拒绝 no-touch。
标记缺失或无法判定时也应拒绝，防止改名或放错目录绕过检查。
保留现有发布保护；该标记用于防误用，不替代签名与构建来源验证。

**验收标准。** 完整门禁后各配置产物身份明确，正式产物不会被测试构建覆盖。
通过真实打包/门禁入口验证：正式产物通过；no-touch、改名的 no-touch 和
缺少必要标记的产物被拒绝，且失败原因确实是产物身份检查。
正式固件的触摸行为仍需对应软件测试及另行授权的实物确认。

依据：[完整门禁](scripts/check.sh)、[构建说明](docs/build.md)、
[发布构建](.github/workflows/release-build.yml)。

### M5：开发规范与实际内存模型不一致

**问题与影响。** 原规范将固件描述为禁止堆分配，实际 `main.rs` 配置了
128 KiB 全局堆；原依赖说明和 unsafe 审计又将其限定为 RSA 大整数用途。
2026-10-17 复核确认，FIDO 的 ML-DSA-44、65、87 展开密钥也通过 `Box`
使用该堆。原审查只列出 RSA 的例外范围不完整，容易误导后续开发与审查。

**修正方案。** 保留 `no_std` 和新增代码默认不分配的约束，将 RSA 调用链与
FIDO ML-DSA 密钥包装登记为现有例外，统一说明固定预算、初始化责任、生命周期
及失败边界。RSA 返回后 core1 可能仍有尾部候选，不能用 worker 串行推导后续
FIDO 开始时堆必为空闲；重叠峰值、耗尽和恢复仍需另验。这是记录现有实现，
不授权新增用途，也不以删除 RSA、取消 ML-DSA boxing 或重写算法作为前置步骤。

**验收标准。** 开发指南、架构、unsafe 审计及相关说明一致，源码和固定目标的
依赖配置核对结果支持登记的用途，已有分配器和栈区域守卫继续有效。规范纠偏
与运行时资源验证分开报告；完整门禁不等于堆容量、无泄漏或可恢复耗尽证明。
文档和注释本身的修正无需增加固件行为版本。

依据：[开发规范](AGENTS.md)、[贡献指南](CONTRIBUTING.md)、
[堆配置](firmware/src/main.rs)、[依赖说明](firmware/Cargo.toml)、
[unsafe 审计](docs/unsafe.md)。

## 4. 推荐处理顺序

| 步骤 | 工作 | 完成条件与后续关系 |
| --- | --- | --- |
| 1 | M4：隔离产物并补齐打包校验（已完成离线验收） | 各类检查明确审计哪个镜像，错误产物在实际入口被拒绝，为后续实验固定产物身份；结果见第 7 节 |
| 2 | M5：同步内存规范（已完成离线验收） | 已登记 RSA、FIDO ML-DSA 例外及共享资源边界；资源实测独立待验，见第 8 节 |
| 3 | M3：先审查安全调用点，再重构存储契约 | 功能修复、测试及原始日志保留；记录修正后的 check.sh 已从头完整通过。既有安全回归 state 未完成，资源优化独立作为后续工作，见第 9、10 节 |
| 4 | M2：验证并确定日用发行策略 | 严格配置的授权和兼容性结果清楚，再同步发布选择与用户文档 |
| 独立推进 | M1：准备并执行硬件验收 | 准备可提前进行，实测使用明确配置的产物；受影响软硬件改变后重新确认适用证据 |

步骤 1、2 的范围较小，可以先推进。M2 的兼容性评估也可提前准备，
但不得把推荐配置等同于已完成发布切换。M1 不因软件门禁通过而自动完成，
也不要求为了等待实物而停止独立的软件改进。

## 5. 后续实施与验证要求

- 每步分别记录修改范围、验证命令、实际退出状态和未验证边界，避免把不同
  配置或旧版本的结果沿用为新实现证据。
- 沿用完整门禁：`nix develop -c env HOST_TARGET=x86_64-unknown-linux-gnu ./scripts/check.sh`。
  协议、客户端或配置行为受影响时，补跑对应客户端与构建配置；完整门禁不能
  替代硬件、互操作及新守卫实际入口的验证。
- 新增守卫应在其实际门禁行或打包入口注入反例，检查失败原因；单独测试 helper
  返回错误不足以证明接线有效。
- 固件行为变化按仓库规则递增 `bcdDevice`；用户可见行为同步指南和 CHANGELOG，
  协议及持久格式变化另按既有契约处理。纯文档、构建隔离或保持行为的重构不据此
  自动递增固件版本；镜像内新增标记的具体影响在实现时判定。
- 本文没有授权刷写、签名、OTP、断电注入、提交或推送；相关操作仍遵循
  [AGENTS.md](AGENTS.md) 和 [SAFETY.md](SAFETY.md)。

## 6. 本次记录的验证边界

原始五项结论来自工作树中的说明和源码核对，当时建议方案尚未实现。
本轮未连接设备，也没有新的硬件或漏洞复现结果。
对本文的文档检查和工作树门禁结果另在交付说明中报告；即使门禁通过，
也不表示 M1–M5 已解决。

## 7. M4 软件实施与验收

2026-10-16。M4 已完成离线验收，原始完整门禁最终退出 0。配套文档统计、
引用和生成区已同步；剩余行预检发现一个 M4 前已存在的阈值问题，经用户明确
允许，将 `CORPUS_FLOOR` 从 50 提高到 51 后完成验收。

### 修改范围

- 正式触摸镜像保留在 `target/thumbv8m.main-none-eabihf/release/firmware`。
  16 MB、指纹候选、显示和 no-touch 分别使用
  `target/{16mb,fingerprint,display,no-touch}/`，CI 各配置也有独立目录。
- 尺寸、栈、调试命令、分区表和临时签名检查显式接收对应 ELF；常量时间、
  ELF 和 owner 守卫仍审计原来的正式镜像。复现入口同步识别带路径的检查行。
- 构建脚本从实际 Cargo feature 生成 24 字节可加载只读 `.rsk_build` 标记，
  包含格式版本、no-touch/fingerprint/display 位和 Flash/KVMAIN 容量。
  未新增运行时 unsafe、固件依赖或持久记录格式。
- `pt.sh` 在产生分区表输出前检查身份；正式打包拒绝 no-touch、缺失、畸形
  或未知标记。`--test-no-touch` 仅接受实际 no-touch 镜像，Nix 四个测试包
  显式选择该策略。发布入口保留文件名保护，并在复制 UF2 前校验对应 ELF。
- 构建规范、使用示例、发布过程摘要和测试清单同步。镜像增加元数据，按仓库
  镜像构建计数守卫将 `bcdDevice` 从 `0x09E2` 递增至 `0x09E3`。

### 已观察的验证结果

| 验证 | 实际结果与判定依据 |
| --- | --- |
| 修改前复现 | 退出 1；执行五条真实构建行的 recording Cargo 后，目标目录仅有 1 个而非 5 个 |
| 五类真实固件 | 均成功链接；新测试核对每份 ELF 的 feature 位与 Flash/KVMAIN 容量 |
| `pytest scripts/test_firmware_artifacts.py` | 退出 0，26 passed；覆盖真实打包、UF2 标记保留、配置路径及错误身份 |
| 既有接线/发布/ELF 测试 | 退出 0，329 passed；发布过程生成区已同步后通过 |
| `reproduce.sh --self-test` 与其测试 | 退出 0；135 gate rows；25 passed |
| `actionlint`、`elf_gate.py` | 退出 0；标记位于可加载只读段，原有镜像布局约束通过 |
| Nix 实际 installPhase | 正式/测试策略分别接受对应 ELF，退出 0 并导出 ELF/UF2；交叉输入退出 1，原因为 no-touch 身份不匹配，未产生导出文件 |
| 文档构建与离线链接 | `scripts/docs.sh check` 退出 0；补充构建示例后再次通过 |
| 原始完整门禁首轮 | 退出 1；11C 统计文档仍写 130/48，而新增五条身份检查后实际为总行数 135/Python 行数 53；此前构建及镜像审计均通过，该轮后同步统计，最终重跑结果见下 |
| 剩余行预检 | 已发现引用行号、配置矩阵与派生界限表过期，核对差异后同步；脚本测试退出 1：3761 passed、1 failed，仅既有语料阈值失败 |
| 既有阈值单项复现 | `test_a_floor_tracks_the_tree_rather_than_sitting_at_zero[CORPUS_FLOOR]` 退出 1：阈值 50，小于实际语料 103 的一半（51）；HEAD 只读归档也测得语料 103、阈值 50，确认并非 M4 引入。经用户允许提高到 51，未降低或跳过检查 |
| 获准修正后的声明守卫 | 原失败项退出 0，1 passed；实际 `claims_gate.py` 退出 0；整个 `test_claims_gate.py` 退出 0，133 passed |
| 原始完整门禁最终重跑 | 退出 0，`ALL CHECKS PASSED`；脚本测试 3762 passed，CLI 312 passed，互操作 45 passed，泄密扫描通过；日志 `target/m4-check.log`，首轮失败日志为 `target/m4-check-first.log` |

新守卫的反例在临时副本通过真实入口运行，未修改正在验收的工作树：

| 接线反例 | 实际结果 |
| --- | --- |
| 原实现正向控制 | 退出 0，3 passed |
| 删除 `pt.sh` 的身份检查调用 | 退出 1，2 failed；原名和改名 no-touch 均被错误接受（返回 0），触发“应拒绝”断言 |
| 将正式身份门禁行替换为 `true` | 退出 1，1 failed；实际行错误接受 no-touch，返回 0，触发“应拒绝”断言 |

命令在固定 Nix 开发环境执行。当前环境需临时启用
`NIX_CONFIG='experimental-features = nix-command flakes'`；未修改全局配置或 pins。
针对性日志为 `target/m4-artifacts-tests.log`、`target/m4-controls.log`、
`target/m4-nix-package.log` 和 `target/m4-docs.log`。

完整门禁的等效复验命令（本轮将输出重定向至上述日志）：

```sh
NIX_CONFIG='experimental-features = nix-command flakes' \
  nix develop -c env HOST_TARGET=x86_64-unknown-linux-gnu ./scripts/check.sh
```

### 验证边界

Nix 验证执行的是实际求值后的安装阶段，输入为本轮 Cargo 构建的真实 ELF，
不是完整 hermetic `nix build` 或 bit-identical 重建。未触发远端 CI，未连接
硬件，未确认实物触摸行为。截至本节验收时，M1、M2、M3、M5 仍未实施；没有刷写、生产签名、
OTP、提交或推送。完整门禁中既有的随机临时密钥/临时文件检查沿用原授权。

## 8. M5 内存规范纠偏与后续资源验收

2026-10-17。用户授权按前述方案修复 M5，要求意外情况暂停询问，并记录做了
什么及原因。开工基线为 `ec729e13`，工作区干净。本轮范围是规范、架构与相关
说明的纠偏，现已完成离线验收，原始完整门禁实际退出 0。运行时资源验证
继续独立待验，未将规范修正等同于资源安全结论。

### 做了什么，为什么这样改

- 将 `AGENTS.md`、`CONTRIBUTING.md` 的绝对禁堆表述改为新增代码默认不分配，
  明确现有 RSA 和 FIDO ML-DSA 例外；新增用途或扩大预算仍需理由及资源、
  生命周期、失败路径证据，避免把已有分配器变成任意扩用的许可。
- 在[架构内存模型](docs/architecture.md#memory-model)集中记录单一 128 KiB
  全局堆、BSS 预留、双核共享、RSA 调用链和三种 ML-DSA 密钥包装。算法库
  `rsk-mldsa` 自身无分配，与外层 `Box` 分配并不矛盾。
- 核对 RSA 返回与 worker 后续调度，纠正“返回时 core1 已停好”和“FIDO 期间
  堆必为空闲”的注释。源码允许 core1 在后台完成最后一个候选，资源验收必须
  覆盖其与下一请求的重叠；本轮没有复现新的耗尽故障。
- 同步 unsafe 审计、安全论证、依赖说明、分配器符号与秘密残余的解释，以及
  第一阶段记录的补注。不再把“仅 RSA 使用堆”或缺少某个释放符号当作生命周期
  保证；保留库内部临时值的既有残余、所有检查条件及假设状态。
- 将堆初始化前的调用链、资源峰值、碎片、回收和耗尽恢复继续列为待验证，
  不把固定容量、布局测试、主机分配器或完整门禁当作设备运行时证明。

本轮没有改变分配器、堆容量、运行时控制流、依赖、协议或持久格式，
没有新增守卫或放宽门禁；`bcdDevice` 保持 `0x09E3`。保留已有 ML-DSA boxing，
避免恢复历史栈溢出；直接扩大堆会挤占栈，全面无堆则涉及密码学实现与内存
所有权重构，均不作为这次规范修正的前置步骤。

### 历史依据与本轮核对

本地 Git 历史不是浅克隆。以下日期按北京时间，历史测试结果仅作当时的记录，
不充当当前版本的实测：

| 提交 | 核对到的依据 |
| --- | --- |
| `8a7d3fea`，2026-06-12 | 初始公开代码和 unsafe 说明已有 64 KiB RSA 堆，早于贡献规范的绝对禁堆表述 |
| `4062294c`，2026-06-12 | 首次新增该规范，提交说明未给出专门的禁堆论证 |
| `f7b89ea2`，2026-06-13 | 记录双核 RSA 尾部和证书签名重叠耗尽 64 KiB、panic 停机，随后改为 128 KiB；本轮未复现实物事故 |
| `6c62a613`，2026-06-21 | 为修复 ML-DSA-44 签名栈溢出，将展开密钥改为 `Box`；当前三种参数集延续这一存放方式 |
| `d261c5e2`，2026-06-24 | AGENTS 加入同样规则时，上述两类用途已经存在 |
| `00c3591b`，2026-08-29 | 分配器守卫提交明确将禁分配解释为对新增代码的约束，而非现有树完全无堆 |

直接读取了 `main` 初始化、`core1` 搜索及尾部清理、worker 调度、`rsk-rsa`
的大整数/字节转换/生成结果、FIDO `CredKey` 的构造与布局测试、ML-DSA 展开
密钥的 `ZeroizeOnDrop`，并核对 applet、device 和 display 层的 RSA 结果传递。
固定目标的 feature 图用于区分依赖配置与调用用途；启用 `alloc` 的依赖不等于
其每条路径都分配，主机 proc-macro 的分配也不等于设备分配。

### 验证记录

| 验证 | 实际结果 |
| --- | --- |
| 默认固件固定目标依赖核对 | 退出 0；`cargo tree --locked --offline -p firmware --target thumbv8m.main-none-eabihf -e normal,features`，日志 `target/m5-features-default.log` |
| display、fingerprint 固定目标依赖核对 | 相同命令分别追加 `--features display` / `--features fingerprint`，均退出 0；日志 `target/m5-features-display.log`、`target/m5-features-fingerprint.log` |
| 文档构建与离线链接 | `scripts/docs.sh check` 退出 0，0 个链接错误；日志 `target/m5-docs.log` |
| 根目录规范及记录链接 | `lychee --offline --no-progress AGENTS.md CONTRIBUTING.md RSKEY-MACRO-REVIEW-2026-10-16.md RSKEY-FPM583F-PHASE-1-RECORD.md` 退出 0，0 个链接错误；日志 `target/m5-record-links.log` |
| 注释及配置语义核对 | 退出 0；四个 Rust 文件的非注释非空行、固件 manifest 和镜像注册表的 TOML 数据、ELF 守卫去除模块说明后的 AST 均未改变；秘密注册表只改变指定残余的解释，scope 与 verdict 未动。日志 `target/m5-source-review.log` |
| 相关守卫预检 | 整体退出 1，原因是缩短 `rsk-fido/src/lib.rs` 的注释后固定引用整体移动一行；版本、文档常量、平台、秘密、owner 和说明计数守卫均退出 0。随后保留原声明位置，未改引用锁或检查条件。日志 `target/m5-preflight.log` |
| 引用复验 | `python scripts/citation_gate.py` 退出 0，1517 个引用通过；日志 `target/m5-citations.log`，完整门禁中再次通过 |
| 默认 ELF 与栈区域 | ELF 守卫通过，3 个登记的分配器符号、1 个分配器声明；默认栈区域 174 KiB、display 170 KiB，均满足 168 KiB 下限；五类镜像构建及身份检查均通过。这是静态布局检查，不是运行时峰值测量 |
| 假设与秘密登记 | 完整门禁通过；平台仍为 92 项假设、5 项 accepted-risk、9 项 discharged、78 项 pending；秘密清单仍为 57 项，未提升证据状态 |
| 原始完整门禁 | 实际退出 0，`ALL CHECKS PASSED`；脚本测试 3762 passed，CLI 312 passed，互操作 45 passed，泄密扫描通过；日志 `target/m5-check.log` |

命令在固定 Nix 开发环境执行。本机继续临时启用 experimental features，
没有修改全局配置或 pins。完整门禁的实际命令为：

```sh
NIX_CONFIG='experimental-features = nix-command flakes' \
  nix develop -c env HOST_TARGET=x86_64-unknown-linux-gnu ./scripts/check.sh
```

### 后续资源验收与边界

后续独立验证应绑定板型、构建参数和镜像哈希，至少覆盖：

| 验收场景 | 需要取得的证据 |
| --- | --- |
| 所支持规格的 RSA 生成、导入和私钥运算 | 堆/栈组合峰值及两核重叠时的资源余量 |
| ML-DSA-44、65、87 创建与签名 | 各参数集堆/栈组合峰值，以及密钥布局保持正确 |
| RSA 后立即进入 ML-DSA 请求 | 后台候选尾部与下一命令重叠的容量与响应结果 |
| 连续成功、错误及支持取消的路径 | core1 静止后回收情况、持续占用及碎片边界 |
| 分配失败和恢复 | 实际停机/USB 行为、持久状态及恢复条件；不预设能返回正常协议错误 |
| 启动到堆初始化前的调用链 | `PLAT-UNSAFE-003` 所欠的依赖初始化路径审阅；当前状态仍为 pending |

本轮未连接硬件或进行故障注入，不新增设备操作授权；未触发远端 CI，
没有刷写、生产签名、OTP、提交或推送。M1、M2、M3 和 E1 的安排保持原状。
规范纠偏验收与上述资源实测分别记账，后者未完成时不宣称整机内存安全已验收。

## 9. M3 存储读取错误契约与凭据一致性修复

2026-10-17 开工，2026-10-18 续验。基线为 `026cdb89`，开工工作区干净。
用户授权开始修复，要求遇到意外情况或需要取舍时暂停询问。2026-10-18 收尾范围
调整为区分缺失与读取失败，修复故障下错误的新建、删除及安全决策，保留现有修复、
测试和原始日志。本轮记录修正后的当前工作区完整门禁已从头无筛选通过，退出 0。
重型 HMAC 证明作为既有安全回归，资源失败与 M3 功能缺陷分别记账，见第 10 节。
state 层继续标为“未完成”，不能据部分结果宣称全部验收通过。

### 已复现的问题与修改

- 同一 RP、同一用户重新注册时，`credential_store` 跳过读取失败的旧凭据，
  会另建一条记录。改为保留读取不确定性：能找到可读的同一账号时仍可替换，
  否则在任何持久修改前拒绝分配新槽，避免重复凭据；恢复后可正常重试。
- 单个凭据删除时，RP 索引读取失败会被跳过，删除仍返回成功，留下偏大的计数。
  CTAP 和屏幕入口现共用删除路径，先读取本次所需的 RP 索引，再执行已有的
  “凭据先删、RP 后更新”顺序，并传播索引更新失败。
- 对“索引读失败时优先删凭据，还是整次拒绝”的取舍，用户明确选择整次拒绝、
  保留凭据并在恢复后重试。写入仍非事务：删除过程中写入失败或断电可能留下
  孤立的 RP 索引；本次未增加持久事务日志或声称任意故障都可回滚。
- `Storage::read`、`size` 改为直接返回 `Result<Option<usize>>`，移除独立
  `last_error()` 旁路；`exists` 传播错误。设备后端、共享故障夹具、各库测试、
  fuzz 与 Miri 后端同步迁移，空记录与缺失继续分别是 `Some(0)` 和 `None`。
- `Fs::try_size` 开放给调用者，元数据更新与缓存直接消费底层的返回错误。
  既有 `Fs::read/size/has_data` 便利接口的有损契约仍保留；这次修改不宣称
  类型系统已阻止所有未来调用者误用它们。PIN、种子初始化、授权、备份与
  重置路径已有的 fallible / restrictive 处理继续由对应测试验证。
- 无新增生产依赖、堆分配或 unsafe；协议字段、持久布局及 PIN/passkey 派生
  保持原样。因故障处理行为改变，`bcdDevice` 从 `0x09E3` 递增至 `0x09E4`。
  同步用户指南、威胁模型、删除策略登记、形式化突变锚点与源码引用。

### 已观察的验证与环境处理

| 验证 | 当前实际结果 |
| --- | --- |
| 修改前的项目内回归测试 | 退出 101；两项分别因故障下发生持久修改、删除被错误报告成功而失败；可读账号替换对照通过，日志 `target/m3-repro.log` |
| 第一阶段 FIDO 库测试 | 退出 0；752 passed、1 ignored，日志 `target/m3-fido-tests.log`；后续增加的场景亦随下述完整门禁通过 |
| 底层契约迁移后的主机工作区测试 | 退出 0，日志 `target/m3-host-tests.log`；后续补充的空记录与删除失败场景亦随下述完整门禁通过 |
| 删除策略与突变接线预检 | 删除登记为 44 个调用点，突变接线为 80 patch / 5 unreachable，均通过；共享删除后少一个重复调用点，未降低检查条件 |
| 源码引用同步 | 按源码内容迁移引用，再重建引用锁；初次 1517 个引用通过，后续补齐共享删除引用并纠正缓存判定名称后，1518 个引用通过。续验补齐 SEC-FIDO-005 的旧行号、显式文件名及文件总数后，1522 个引用通过；未修改引用守卫或证据状态；日志 `target/m3-sync.log`、`target/m3-final-preflight-1.log`、`target/m3-record-citations-check.log` |
| Kani 环境 | 首次 Nix 环境检查退出 101，缺少命令入口；用户选择按仓库固定版本配置。已有匹配资源与 nightly，离线安装 `kani-verifier 0.67.0` 到用户目录后版本检查退出 0；未使用管理员权限、未修改项目 pins |
| 此前完整门禁 | 最终退出 0，`ALL CHECKS PASSED`；3769 项脚本测试、312 项 CLI 测试、45 项互操作测试通过，五类固件构建、身份与镜像检查及泄密扫描通过；日志 `target/m3-check-final.log`。前两轮分别因文档计数、脚本固定计数未同步退出 1；日志 `target/m3-check.log`、`target/m3-check-pre-count-sync.log`。计数同步的 15 项复验亦通过，日志 `target/m3-counter-recheck.log` |
| 收尾完整门禁首轮 | 退出 1，停止原因为生成文档 `docs/assurance-bounds.md` 的一行仍引用旧位置；核对差异后按原生成器同步，原边界检查通过。失败日志 `target/m3-check-scope-final.log`、运行记录 `target/m3-check-scope-final-run.json` 保留 |
| 记录修正后的当前工作区完整门禁 | 退出 0，`ALL CHECKS PASSED`；3769 项脚本测试、312 项 CLI 测试、45 项互操作测试通过，主机与 feature 测试、固件构建及镜像检查、1522 处引用、397 项边界和泄密扫描等均通过。完整复跑约 1295 秒；日志 `target/m3-check-scope-final-2.log`，命令、参数及退出结果见 `target/m3-check-scope-final-2-run.json`。本轮改动文件（审查记录除外）的运行前后哈希一致 |
| Kani `pr` | 首轮元数据证明达到 5 分钟上限；完成下述定位并获用户同意后，完整 `pr` 已通过 68 个证明及原有覆盖检查；日志 `target/m3-kani-final.log` |
| Kani `state`（既有安全回归） | 未完成；重型 HMAC 证明发生主机 OOM，基线与修复版的独立证明对照均通过，但不能替代完整层级。资源证据和后续安排见第 10 节 |
| 审查记录链接 | 本轮拆分功能记录与资源后续后的检查、最终结果状态更新后的复查均退出 0、0 个链接错误；最终为 28 个链接、21 个唯一链接，26 OK、2 Excluded。日志 `target/m3-record-links-scope-final.log`、`target/m3-record-links-closeout.log` |

### Kani 超时定位与等价改写

用户选择先对比基线、定位原因，不提高上限或放宽证明。相同的 Kani 0.67.0、
软件密码学后端与 5 分钟上限下，将 `026cdb89` 和当前代码放入临时副本，
分别使用独立构建目录。早先共用目录时 Cargo 复用了基线产物，那次当前版本的
通过结果作废，不计入下表。

| 同一删除证明的实现或写法 | 实际结果 |
| --- | --- |
| 基线 | 退出 0，约 0.21 秒；`target/m3-kani-baseline.log` |
| 当前 `Result<Option<usize>>` 实现 | 退出 1，达到 5 分钟上限；`target/m3-kani-current.log` |
| 仅展开测试后端的 `read` → `size` 转调用 | 仍超时；`target/m3-kani-read-inline.log` |
| 仅将生产函数的 `?` 改为完整 `Result` 匹配 | 仍超时；`target/m3-kani-result-match.log`；未合入该生产改写 |
| 在证明中按原布尔输入分别调用同一生产函数 | 删除证明约 0.31 秒、添加证明约 0.56 秒通过；`target/m3-kani-split-both.log` |

GOTO 中间代码与运行日志显示，合并后的嵌套枚举路径让 CBMC 展开本应不可达的
元数据循环；显式分支消除了这项开销。候选只改证明中的调用组织，保留原有
13 个断言、3 个假设、4 个覆盖点和 FID 域，生产代码与超时不变。分别注入
`BugMetaAddDropsOnFault`、`BugMetaDeleteDropsOnFault` 后，对应故障断言仍失败，
不是正常操作被拒绝的反向失败；日志 `target/m3-kani-candidate-mutants.log`。
用户明确选择采用此等价改写后才应用到工作区，完整层级验证仍以脚本最终结果为准。

### Token 守卫的额外修复

预检发现新增的 `resident_delete::delete(fs, slot, ...)` 会让原有 token 守卫
把 `&mut Fs<S>` 推断成 FID 类型，进而误报未修改的 `migrate_keydev_pin`。
用户选择修复守卫，而非将私有助手改名绕过。先补回归测试，实际观察到同一个
规则还漏掉关联函数写入经过中间助手的真实 FID；日志 `target/m3-token-repro.log`。

守卫现按调用形式定位参数：普通方法的首参数是 FID，关联函数先消费显式接收者。
类型推导、直接写入、局部变量和扫描范围外的写入共用该规则，保留类型别名、
限定路径、泛型和嵌套接收者表达式。没有增加函数名豁免或改变原有检查下限。
128 个相关测试通过，实际源码的 token 统计恢复为 `persistent=15/5`，其余轴不变。

全量脚本回归另检出两处随本轮变更过期的固定输入：测试函数总数仍为 2672，
文档统计反例仍替换旧的 212。核对实际新增的两个测试函数、无删除后，将总数
同步为 2674，并让反例把当前的 215 改为错误的 216；保留等式检查和原失败方向。
15 项相关复验通过，未降低守卫下限或改变规则；最终无筛选完整门禁也已通过。

### 门禁反例

以下前七项在包含未跟踪新文件的临时源码副本中，执行 `check.sh` 原样的
`run_tests "test (host)" ...` 行。每项均退出 101，确认是对应断言失败，非编译错误。
日志汇总为 `target/m3-entry-mutations.json`。

| 突变 | 实际失败方向 |
| --- | --- |
| 删除去重的不确定性拒绝 | 读取故障下发生持久修改，写入计数 4 而非 0 |
| 删除 RP 预读失败的拒绝 | 读取故障仍被报告为删除成功 |
| 屏幕入口丢弃删除结果 | 预读失败及 RP 删除失败均被误报为成功 |
| `BugCacheFaultAsAbsent` | 缓存故障后权限配置不再保持 `NO_CAPS`，实际为 571 而非 0 |
| `BugMetaAddDropsOnFault` | 重建空元数据丢掉旁观记录，且未按要求拒绝 |
| `BugMetaDeleteDropsOnFault` | 故障下虽清理了范围，却丢失应返回的元数据失败 |
| `BugDeleteRpBeforeCred` | 写入预算 2 时留下仍存活而无 RP 索引的凭据 |

守卫修复另经 `check.sh` 原样的 `pytest (gate scripts)` 行验证，此处仅用
`PYTEST_ADDOPTS=-k token_refinement_gate` 选择相关回归，未将其计作完整脚本测试通过。
绿色对照退出 0；撤销参数识别修复、删除关联函数识别两种突变均退出 1，分别由
误报对照和真实写入漏报断言捕获；日志 `target/m3-guard-row-mutations.json`。
最初的临时 `no-checkout` 克隆缺少索引，使版本守卫看不到新增发布记录，该轮已
作废中断；建立临时索引后才取得上述有效结果。最终门禁将无筛选执行所有测试。

### 证据记录补齐与当前边界

续验的最终 diff 审核发现 SEC-FIDO-005 证据记录仍有旧的回滚行号、删除顺序
引用和文件总数摘要。暂停确认后，用户选择补齐记录并继续验证。现引用实际的
877/894/900 三处回滚、898 的凭据写入及共享删除中 23→29 的调用顺序；
写入涉及四个文件，连同读取者共五个。补全文件名后，原有引用守卫可识别原先
漏检的简写位置；引用锁仅新增逐行审阅的三个位置，1522 个引用检查通过。
收尾首轮门禁发现同一组修正还需同步到 `docs/assurance-bounds.md` 的生成行。
已核对只有这一行的引用文本不同，再通过原生成器更新并通过边界检查；未改
生成器、界限数值或证明条件。差异与结果保存在
`target/m3-bounds-record-sync.diff`、`target/m3-bounds-record-sync.log`、
`target/m3-bounds-record-check.log`。

此前完整门禁已无筛选通过。运行前已清理本轮共用过的两个主机包缓存以避免
复用突变产物，完成独立比较后也清理了比较用构建缓存；源码快照、验证日志及
嵌入式默认镜像保留。本轮已对记录修正后的当前工作区重新完整运行门禁并观察到
退出 0；完整门禁与 Kani 运行脚本均未修改，所有原始日志保留。
Kani pr 与独立对照的成功均不替代尚未完成的 state 层。

未连接硬件，未刷写、生产签名、写 OTP、提交或推送。完整门禁是离线软件验收，
不补齐 M1 的硬件实测，也不建立任意写入故障下的事务性回滚保证。

## 10. Kani 既有安全回归的资源问题（独立后续工作）

**当前状态：未完成。** 2026-10-18 用户调整工作范围：暂停本机追加交换空间
和修改 Kani 执行器的尝试，资源优化单独留作后续工作。保留以下原始实验结果、
日志及候选，不将 OOM 当作 M3 功能失败或证明通过。本轮不继续资源实验。
既有临时诊断助手另存于 `target/m3-resource-followup/`，文件哈希见其中的
`archive-sha256.json`；原始日志和候选结果仍保留在各自原路径，未覆盖或删除。

后续须在资源足够的环境按原条件完整复跑 `./scripts/kani.sh state`，保持仓库
固定工具版本、原编译参数、证明条件、断言、覆盖要求和超时。当前原脚本仍要求
至少 31 个证明、36 个覆盖点，单证明上限 60 分钟，死覆盖副本上限为 1。
环境未具备或整层未完成时，状态保持“未完成”；不得以单独证明、隔离候选或
完整 check.sh 通过替代该层结论。遇到新的功能失败或需要扩大范围时暂停询问。

### 原始资源定位与复跑记录

`state` 的资源失败发生在约 268 秒的符号执行后，日志未报告任何失败断言，
Kani 的提示为 `CBMC appears to have run out of memory`。当时独立的 Kani
驱动和求解器合计已占用数 GiB，不能将这个结果记作证明通过或代码反例。
用户选择先单独运行失败证明并对比基线，保持条件和原有 60 分钟上限。
两轮使用独立源码与构建目录、相同参数和软件密码学后端，逐次运行：

| 版本 | 实际结果 | 证明耗时 | 进程组合计 RSS 峰值 |
| --- | --- | --- | --- |
| `026cdb89` 基线 | 退出 0；1334 项检查无失败，1 个覆盖点满足 | 501.10 秒 | 9.3 GiB |
| 当前修复 | 退出 0；1334 项检查无失败，1 个覆盖点满足 | 502.96 秒 | 9.4 GiB |

两轮的授权失效、MAC 重放断言均为 `SUCCESS`，合法授权覆盖为 `SATISFIED`。
证明文件 SHA-256 相同，包含状态、描述及位置的完整检查明细也逐字节相同。
日志为 `target/m3-kani-state-{baseline,current}.log`，参数与采样保存在对应的
`-driver.log` / `-run.json`，汇总为 `target/m3-kani-state-comparison.json`。
内存数字是同一时刻的进程组合计，未将不同时间的单进程峰值相加。

Kani 0.67.0 的 [runner](https://raw.githubusercontent.com/model-checking/kani/kani-0.67.0/kani-driver/src/harness_runner.rs)
会保留各证明结果至整轮结束，而 [结果类型](https://raw.githubusercontent.com/model-checking/kani/kani-0.67.0/kani-driver/src/cbmc_output_parser.rs)
包含执行轨迹。这与整层运行中驱动内存累积的观察一致，是资源失败的主要嫌疑；
单独证明通过不能替代整层的 31 个证明与 36 个覆盖点检查。此前用户选择临时增加
4 GiB 交换空间，保持原脚本、证明条件和上限，曾通过系统密码窗口启用
`target/m3-kani-state-4g.swap`，权限为 0600，原有交换空间未改动。本机会话没有
Polkit 认证代理，恢复登录会话的显示环境后，使用现有 `sudo` 图形密码入口授权。
2026-10-18 完成 root 所有权修正后原样运行 `./scripts/kani.sh state`，
MAC 授权证明通过，耗时约 359 秒；同一失效证明仍报告内存耗尽，内核记录
`cbmc` 进程 697930 被 OOM 杀死，未产生断言反例。按约定中断自有进程组，
整轮退出 130，未执行到最终证明及覆盖门槛；停止时累计约 1152 秒。
采样的进程组合计 RSS 峰值为 9062048 KiB，约 8.64 GiB，不含已换出页，
不能读作内存总需求。参数、采样及内核证据分别保存在
`target/m3-kani-state-with-swap-{driver.log,run.json,kernel.log}`。

专用交换文件已通过系统密码窗口关闭，再核对不在 `/proc/swaps` 后删除；
原有 `/swap.img` 保留，未修改持久系统配置。清理记录为
`target/m3-swap-cleanup.json`。用户选择先设计并验证逐证明进程隔离方案，
保留全部证明条件和覆盖门槛，验证后再决定是否应用到仓库脚本。

### 逐证明隔离候选的实际结果

用户选择先设计并验证进程隔离，保持全部证明条件和覆盖门槛，未直接修改仓库
`scripts/kani.sh`。Kani 0.67.0 的 JSON 名单列出原有 31 个标准证明，
无契约证明；候选逐项保留原包选择、编译参数和 60 分钟上限，仅添加精确选择。
每次必须报告指定的唯一证明通过，再将各次计数累加；原有 31/36 门槛、覆盖
解析和死覆盖副本上限保持不变。名单与 97 个输入文件的哈希分别保存在
`target/m3-kani-isolated-list.json`、`target/m3-kani-isolated-inputs.json`。

候选实际判定行经过 15 项模拟输出或执行器突变实验，正向对照和合法死副本
通过；空名单、重复名单、计数不一致、错误证明名、零证明摘要、重复证明摘要、
缺明细、进程失败、未达覆盖、缺覆盖、两个死副本及两种漏跑均按对应原因拒绝。
结果为 `target/m3-isolated-candidate/row-checks.json`。覆盖丢失实验的初版夹具
同时删除了全部明细，触发的是明细拒绝而非覆盖门槛，已作废；修正为保留普通
断言后，实际由 35 低于 36 的门槛拒绝。无效记录单独保留，不计作覆盖门槛证据。
最初列举命令把 `-Z stubbing` 放在父命令，子命令未激活该标志而编译失败；
按 list 的参数作用域传入后列举成功，两次日志均保留，均不算证明运行。

额外交换清理后，真实候选首项 `no_authorization_bypass_begin_mac_is_the_flag`
运行约 298 秒后退出：Kani 命令为 101，候选整行为 1。内核在 2026-10-18
01:55:43 记录 OOM 杀死 `kani-driver` 709080；前端因无退出码报错，未生成
完整证明结论或断言反例，不能将其记为通过。进程组合计 RSS 采样峰值为
9054176 KiB，约 8.63 GiB，不含已换出页。实际结果说明仅按证明隔离仍不足以
完成当前资源配置的验证；整层累计不是唯一需要处理的资源压力。
日志为 `target/m3-kani-state-isolated{.log,-driver.log,-run.json,-kernel.log}`。
候选已自动停止，并确认无自有进程残留；临时交换仍已移除。按当前收尾范围，
本机资源优化与候选执行器尝试均暂停，后续完整 state 复验等待资源足够的环境。

## 11. M2 授权、恢复回归与严格日用发行

2026-10-18。开工基线为 `09921fc1`，工作区干净。用户授权按计划推进 M2，
遇到意外或需要抉择时暂停。核对写入入口后，在严格模式模拟器编译失败处暂停；
用户随后授权补齐其特性入口和依赖接线，并继续验证；选择日用发行启用
strict-config、提供 2 MB 指纹严格包、宽松版显式命名为兼容包。额外核对发现
模拟器未执行 HID FIDO2/U2F 开关后，再次暂停并获得补齐及恢复回归的授权。
以下区分初次评估与最终发行改动；最终完整门禁已从头通过，先前中断和失败日志保留。

### 入口与授权要求

| 入口及影响 | 兼容配置（裸 Cargo） | `strict-config` | 本轮证据 |
| --- | --- | --- | --- |
| CCID Management `WRITE CONFIG`：DeviceInfo / 应用开关 | 无触摸、PIN 要求 | 物理确认；不要求 FIDO PIN | 真实 yubikit 的同意 / 拒绝对照；相关配置读回 |
| FIDO vendor `CONFIG_WRITE`：DeviceInfo、PHY、LED；CTAPHID CBOR 和 CTAP-over-CCID | 无触摸、PIN 要求 | 物理确认；已设 clientPIN 时还需有效 `acfg` token | 三目标、两传输的缺 token、错误权限、错误 MAC、拒绝确认与合法请求对照 |
| 原始 CTAPHID Management `0x43` | 无确认持久写入 | 不支持 | 真实 yubikit 报 `INVALID_COMMAND`；新增模拟器回归验证不落盘 |
| OTP `SET_DEVICE_INFO 0x15`：键盘 HID / CCID OTP applet 共用 | 无确认持久写入 | 空成功、不修改记录或 pgmSeq | 模拟器实际 OTP job 回归；不冒充真实 USB feature-report 验证 |
| CCID Rescue PHY 写入：身份、接口、硬件配置 | 物理确认 | 物理确认 | 当前 `rsk hw` 的同意 / 拒绝对照；既有 rescue 库测试 |
| CCID Vendor `SET LED`，亦可经 CTAPHID MSG 访问 | 无确认 | 物理确认 | 新增库测试核对拒绝先于实时 LED 变化和持久写入 |
| Management 整机 RESET | 物理确认；指纹接线另禁用该入口 | 不支持 | 源码核对及既有主机测试；本轮未做设备重置 |
| OTP legacy `DEVICE_CONFIG` / `SCAN_MAP` / `NDEF` | 兼容入口；SCAN_MAP / NDEF 另按槽位访问码授权 | 不执行写入；SCAN_MAP 也不应用到输出 | 源码核对及既有 OTP 测试；未验证物理键盘输出 |

`strict-config` 的 FIDO PIN 分支还处理仅设置显示设备 PIN 的情况：在设备上收集
PIN；没有可收集它的后端时拒绝，不能当作未设置。此分支依据源码和既有库测试，
本轮客户端对照未运行显示窗口或实物 PIN pad。配置读取保持可用；旧应用开关
仍被执行，切换严格配置不会自动恢复被禁用的应用。CONFIG_LOCK 不实现授权。

### 兼容性结论与资源边界

- yubikit 5.9.1 的 ManagementSession 按连接类型选择不同写入协议。严格配置下
  CCID 可在确认后修改应用开关；FIDO 管理后端只会发原始 `0x43`，不会自动改用
  带 PIN token 的 FIDO vendor 命令，因此该连接下的管理写入失败。
- OTP 管理后端靠 pgmSeq 确认写入，严格模式的空成功不会形成一次成功的配置
  更新。此项核对了固定环境库源码和模拟器 OTP job，未运行实际 OTP-HID 客户端。
- 当前 `rsk hw` 经 CCID 或 FIDO 均能完成合法写入，并正确报告拒绝。当前
  `rsk led --transport fido` 的无 PIN、带 PIN 和拒绝确认路径也通过；模拟器没有
  CCID LED 硬件后端，故该入口由真实 applet 加记录平台的库测试覆盖。
- 严格配置只收紧列出的入口。经确认的不同配置仍消耗 Flash；未保护的 OTP
  槽位编程仍沿用访问码规则，不能宣称它阻止所有未确认写入或解决全部磨损问题。
  去重不是速率限制，触摸也不等于屏幕展示并确认每个配置字段。
- 初次评估时 `firmware-strict-config` 只有 4 MB 几何，`firmware-2mb` 没有严格特性。
  最终增加独立 2 MB 指纹严格包，不能将 4 MB 包直接用于该板。构建和打包通过
  也不代表已烧录或验证指纹硬件。

### 验证环境修正与回归

模拟器一直依赖显示流程，但没有自己的 `strict-config` 入口。直接启用
`rsk-device/strict-config,rsk-display/strict-config` 后，显示重置引用的
`rsk_device::gates_wiped_last` 因缺少 `rsk-device/display` 被裁掉，编译退出 101。
这不是严格固件已被证明无法构建，也不涉及放宽 wipe gate。

获准后新增模拟器 `strict-config` 特性，转发两处严格特性及显示依赖需要的
`rsk-device/display`。是否打开窗口仍由运行参数决定；默认模拟器的特性不变。
初次在独立文件中新增三个模拟器测试，验证 CTAPHID 和 OTP 配置写入符合所选策略、
合法 CCID 写入可用，并重开存储、模拟掉电重启后核对读回。Vendor 库另补一项
LED 授权回归。此阶段未修改生产 Rust，`bcdDevice` 保持 `0x09E4`。
最终发行改动在固件构建标记中增加 strict-config 位，`bcdDevice` 递增为
`0x09E5`；持久记录、依赖版本、unsafe、堆预算和 `rsk` / TUI 版本不变。

`check.sh` 新增严格模拟器整套测试行，随后增加指纹严格镜像构建和身份校验。
最终文档测量为 138 行、63 行 Cargo、54 行 Python、21 行其他，测试行数为 17；
未改变统计守卫。模拟器说明、CLI、
LED、OTP 指南及 CHANGELOG 已同步。

| 验证 | 实际结果与日志 |
| --- | --- |
| 修改前默认 / 严格 applet 与路由测试 | 分别 1040 / 1022 passed，各 1 ignored，均退出 0；`target/m2-default-tests.log`、`target/m2-strict-tests.log` |
| 严格模拟器错误复现 | 退出 101，E0425，找不到被 cfg 裁掉的 wipe gate；`target/m2-emu-repro.log` |
| 修正后默认 / 严格模拟器整套测试 | 各 125 passed，均退出 0；`target/m2-emu-{default,strict}-tests.log` |
| 真实库及当前 CLI 对照 | 默认 / 严格各 46 passed，无跳过，均退出 0；yubikey-manager 5.9.1、python-fido2 2.1.1；`target/m2-client-{default,strict}-final.log` |
| Vendor LED 库回归 | 默认 / 严格各 17 passed，均退出 0；`target/m2-vendor-tests.log` |
| 门禁记录及特性预检 | 引用 1522、特性矩阵、roster、自检均通过；配套 187 项测试通过，退出 0；`target/m2-preflight.log` |
| 文档构建与离线链接 | 退出 0，0 个链接错误；`target/m2-docs.log` |
| 当前工作区完整门禁首轮 | 退出 1，约 609 秒；停止于证据向量生成文档的两处过期输入计数，未观察到功能断言失败；`target/m2-check.log`、`target/m2-check-run.json` |
| 证据向量差异定位 | 当前工作树和只读 HEAD 归档均仅需把 SEC-FIDO-005 的两处输入计数从 6 改为 7；stale / MODELLED-ONLY 状态不变。差异为 `target/m2-evidence-vector-{proposed,head}.diff` |
| 计数同步后的第二轮门禁 | 新增恢复修复获准后，主动中断本轮进程组；退出 143，不能作为通过结果；`target/m2-check-final-interrupted.json` |
| 最终源码真实客户端 | 默认 / 严格各 46 passed，均退出 0；`target/m2-client-{default,strict}-release.log` |
| 应用关闭缺陷复现 | 修复前真实客户端返回成功而非 0x27；主机回归在“disabled FIDO2 answered HID CBOR”失败，退出 101；`target/m2-disabled-fido-repro.log`、`target/m2-disabled-apps-host-repro.log` |
| 应用关闭及恢复回归 | 修复后默认 / 严格整套模拟器各 126 passed；禁用 FIDO2/U2F、重启保持、兄弟应用可用、CCID 重开均覆盖；`target/m2-emu-{default,strict}-recovery.log` |
| 修复后真实客户端关闭测试 | 1 passed，0x27 拒绝符合预期；`target/m2-disabled-fido-client-final.log` |
| 发行相关回归预检 | 320 passed，退出 0；构建目录、实际 ELF 打包、策略拒绝、矩阵与引用自检；`target/m2-release-regressions-final.log` |
| 三个包的真实 Nix installPhase | daily / compat / fingerprint-2mb 的正确镜像均产出 PT ELF 和 UF2；三次交叉替换均在生成 ELF 前按策略拒绝；`target/m2-package-checks.log`、`target/m2-audit/package-results.json` |
| 最终完整门禁 | 退出 0，1326.40 秒，末尾为 ALL CHECKS PASSED；脚本 3774、CLI 312、interop 45 项均通过；57 个变更文件前后哈希一致；`target/m2-check-release-verified.log`、对应 `-run.json` |
| 发行改动后的首轮完整门禁 | 停于 threat-model traceability：宽松策略段落改名后漏同步登记表的固定文本锚点；两条诊断是同一锚点失联 / 新段落未分类。只同步 `where`，保留条目 ID、defence 分类、关联性质和守卫规则；`target/m2-check-release-final.log`、对应 `-run.json` |
| 同步锚点后的后续行预检 | 威胁追踪及后续性质、模型登记通过；停于 slice evidence bundle 的 11 个旧矩阵摘要。不是完整门禁通过；`target/m2-late-preflight.log` |
| 证据摘要同步后的脚本预检 | 3765 passed、9 failed，失败仅为反例仍引用旧矩阵数字；未将缺少待替换文本或额外 bare-integer 断言失败当作有效反例；`target/m2-token-refinement-preflight.log` |
| 同步反例输入后的定向验证 | 19 passed、307 deselected，退出 0；保留所有断言，错误数字继续取自当前同一摘要的其他计数；`target/m2-bundle-fixtures-final.log` |
| 余下门禁行预检 | 文档常量、CLI 312 项、interop 45 项及 gitleaks 均通过，退出 0；仅为后续行，不作为整门禁通过结果；`target/m2-last-rows-preflight.log` |

客户端只连接测试拥有的 `127.0.0.1` 模拟器进程，通过既有 report-only
python-fido2 连接及 CCID 消息桥使用真实协议库，没有 USB 枚举或硬件回退。
确认输入为测试脚本在观察到当前提示后发送的合成输入，不是人或实物确认的证据。
每项使用独立合成存储；错误路径核对具体错误码及相关配置未改变。没有实际运行
ykman CLI 的发现流程、PicoForge GUI、浏览器或 USB/IP，因此不宣称这些端到端兼容。
实验源码为 `target/m2-audit/client_checks.py`，二进制身份见各客户端日志。

新增行的反例在包含新文件的临时源码副本中执行 `check.sh` 原样的
`run_tests "test (emu strict-config)" ...`，日志未筛选：

| 接线反例 | 实际结果及失败方向 |
| --- | --- |
| 原实现正向控制 | 退出 0，125 passed |
| 保留模拟器严格特性名，但去掉 applet 严格特性的转发 | 退出 101，2 failed；原始 CTAPHID 写入被错误接受，OTP no-op 错误推进 pgmSeq；不是合法操作被拒绝 |
| 仅去掉显示依赖特性转发 | 退出 101，E0425，再现原始缺失 wipe gate 的编译错误 |

汇总及完整原行为 `target/m2-audit/row-mutations.json`，实验脚本为同目录
`mutate.py`。突变没有写入工作源码；临时副本和本轮专用编译缓存已清理，日志保留。

补齐应用开关后，通过相同门禁行重做五项反例：控制组 126 passed、退出 0；
去掉严格转发、去掉显示接线、去掉 FIDO2 门槛、去掉 U2F 门槛均退出 101。
后两者分别明确失败于“disabled FIDO2 answered HID CBOR”和
“disabled U2F answered HID MSG”，没有把合法请求被拒绝误当成缺陷被检出。
汇总为 `target/m2-audit/release-row-mutations.json`，原始日志为
`target/m2-audit/release-row-*.log`。实际指纹严格身份门禁行也用改名兼容 ELF
验证拒绝，打包脚本另覆盖两种策略的双向拒绝和通过。

首轮门禁的证据向量差异在未改动的 HEAD 归档也出现，不归因于本轮 M2 实现。
用户随后授权按计划完成，使用原生成器同步这两处计数；不改生成器、证据提交、
证明条件或状态。同步与复验日志独立保留，首轮失败日志不覆盖。

### 应用关闭与恢复

模拟器原先把 HID CBOR / MSG 直接交给 applet，缺少固件已有的 FIDO2/U2F
应用开关门槛。获准后仅在模拟器补齐：关闭 FIDO2 返回 OperationDenied，
关闭 U2F 返回 CONDITIONS_NOT_SATISFIED。固件既有门槛不变。
关闭 FIDO2 后，clientPIN 和 vendor CONFIG_WRITE 也不可达，不能宣称它们
能在同一路径自行恢复。严格配置恢复使用仍可访问的 CCID Management，确认后
重开应用；用户指南明确要求保留可用 CCID。新增回归读回实际存储并模拟重启。

### 已选定的发行策略

- 裸 `nix build` 和 `.#firmware-daily` 启用 strict-config。`nix run .#flash`
  的默认镜像依赖指向 daily；这里只检查接线，不运行刷写程序。
- `.#firmware-compat` 明确保留宽松行为。裸 Cargo 和旧 `.#firmware` 保持兼容，
  不将默认特性的变更隐含施加给原有开发和保证基线。
- `.#firmware-fingerprint-2mb` 使用 `fingerprint,strict-config`、2 MB Flash、
  896 KiB KVMAIN、UART0 GP0/1、GP2 touch、GP3 power；仍是待硬件验收的候选。
- 其余正式发行变体统一启用 strict-config；no-touch 仍仅作测试。下一次发行
  提供 16 个包，主推荐文件为 `daily.uf2`，旧发行不改写。PR preview 保留
  原 `default` 标签，但其 Cargo 参数已是严格配置，并增加显式 compat / 指纹包。
- `.rsk_build` 增加 bit 3。Nix 安装、CI 和发行复制均核对实际 ELF 策略；
  不能通过改文件名把宽松镜像包装成严格镜像。该标记不是签名或来源证明。
- 保证矩阵由原生成器更新。新增构建列保持未证实的 gap；2 MB / 16 MB 原有
  相同特性等价判断因严格特性变化撤回，不将测试通过提升为已有形式证明覆盖。
  新包使包数从 19 增至 22，对应派生数量下限同步上调，未放宽守卫。矩阵为
  40 × 34，1077 个 gap；证据向量同时反映已撤回的等价判断。

Nix 安装阶段验证使用实际 Cargo ELF 和从三个 derivation 读取的 installPhase，
仅将空的 runHook 用于本机夹具。未进行三个包的完整 hermetic Nix 编译或
`--rebuild` 字节复现；这些要求仍由发行及每日 repro workflow 保留。每日 repro
已指向 daily。保留的本机 PT ELF/UF2 未签名，不能宣称是已发布的可复现产物。

严格配置不能替代安全启动、OTP 主密钥或硬件验收，仍保留上文的 Flash 磨损边界。

### 旧证据包的配置摘要（暂停后已获准同步）

新增发行包后，当前矩阵为 34 个配置、1360 个单元，而 11 份证据包的
`result.gate_matrix` 仍把旧的 32 / 1280 及旧 disposition 数量写成当前派生值。
`bundle_gate.py` 按现行树比较该字段，因此预检失败；这些字段的相邻注释本来
要求记录当前派生数量，但它们又位于旧证明包中，不能将旧实测结果改作新结果。

当时按用户“遇到意外或需要抉择先暂停”的要求，先不修改 `assurance/bundle/*.toml`。
已形成仅更新这 11 个 `gate_matrix` 字段、并注明“当前配置元数据复核，历史证明
和日志不变”的拟议差异：`/tmp/rsk-m2-bundle-metadata.patch`；结构化前后值为
`target/m2-audit/bundle-metadata-preview.json`。在内存中调用现有转录守卫，11 项
拟议字段均通过；这不是完整门禁通过，也不是对旧模型或 Kani 证明的新运行。
用户随后明确批准“仅刷新配置元数据并继续”。已原样应用该差异，并解析 TOML
逐份比较，确认每份只有 `result.gate_matrix` 的值改变；其余字段完全一致。
旧 build commit、证明状态、原始日志、哈希、日期、模型和守卫规则均未改变。
实际 bundle gate 已通过，198 个引用数值仍关联到 63 份日志，100 份原始证据
均保留并被引用。后续预检和完整门禁另行记录，不将该通过等同于旧证明重跑。
后续 token completeness 检查还识别了三处旧 `fingerprint` 构建列引用；新增发行包
已让该特性归属 `firmware-fingerprint-2mb` 列。核对三个实际函数及字段用途后，
只同步这三个列名，stutter / out-of-scope 分类、理由和对应代码均不变。
脚本整套预检随后明确报出九个旧数字反例：部分未生成突变，部分因旧数字已不在
当前摘要内而触发额外的 bare-integer 守卫。核对具体失败后，更新测试输入数字，
继续从当前摘要的兄弟计数借值，保证仍由目标 `<count> <noun>` 守卫拒绝；
未删除或放宽断言。相关 19 项定向验证已通过，最终整套验证由完整门禁重做。

### 最终离线验收

在固定 `nix develop` 环境设置 `HOST_TARGET=x86_64-unknown-linux-gnu`，从头执行
未经筛选的 `./scripts/check.sh`。本轮进程正常退出 0；57 个变更文件在门禁运行
前后及验收核对时的 SHA-256 完全一致，未发现漏记的变更文件，HEAD 仍为
`09921fc14b9a9098224dd9d7e5006945e2abdf6a`。原始执行记录为
`target/m2-check-release-verified-run.json`，独立核对为
`target/m2-audit/final-gate-observed.json`。之前的退出 1 和中断 143 均不作为本轮通过依据。

脚本整套为 3774 passed（706.80 秒），CLI 为 312 passed，interop 为 45 passed；
gitleaks 通过。真实客户端的默认 / 严格各 46 项及修复后的单项关闭测试，合计
93 项通过，其传输和模拟确认边界仍以上文为准。门禁通过不将已有 gap / stale
或 MODELLED-ONLY 结果提升为新证明，也不代表 M1、M5 的实物或资源验收。

门禁结束后仅向本审查记录填入验收结果；其余代码、配置与证据元数据保持门禁
结束时的内容。最终文档和记录复核日志为 `target/m2-completion-checks.log`。

本轮没有连接设备、刷写、生产签名、写 OTP、提交或推送；未恢复 M1 实测、M5
运行时资源实验或 Kani state 资源优化。M2 日用发行策略已完成离线验收；整机安全
结论、实物兼容性及正式发行执行不在本次结果内。
