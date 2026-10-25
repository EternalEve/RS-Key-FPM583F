<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# 09F0 长按诊断与依赖安全更新：已部署，长按故障复现

日期：2026-10-24，Asia/Shanghai。接续
[09EE 实机失败记录](RSKEY-FPM583F-AUTO-REARM-09EE-VALIDATION-2026-10-23.md)：
连续十次触摸通过，但窗口前长按后，该请求和后续独立新请求都超时。该失败记录保留。

本轮已获固定镜像授权，代码分区写入及独立读回通过。普通触摸 2/2、BOOTSEL、
取消和正常超时对照通过；窗口前长按后的重新触摸、后续独立新请求均返回 `0x2F`。
长按故障未修复。11 组原始分页重放与解码一致，63 条观察连续，缓冲未溢出。

## 本轮结果及边界

`09F0` 保留显式 `fingerprint-touch-trace` 测量特性，固定 RAM 环形缓冲记录最近
128 个 GPIO、确认窗口和控制器观察；读取时另采实际 SIO 电平。新 vendor `0x16`
返回 52 字节分页，原 `0x15` 六字节状态保持不变。普通固件不分配环形缓冲并返回
`6D00`。主机 `rsk` 为 0.3.39，新增 `rsk fingerprint trace`，拒绝拼接变化中的分页。

触摸控制器状态转换保持原样。记录和依赖补丁都会改变执行时间，必须重新观察是否能
重现故障。本轮未声称修复长按问题，也未创建凭据、改变 PIN 或录入、删除指纹模板。

旧 `09EF` 未获授权、未部署；其包和失败门禁日志保留。中断期间 Git HEAD 从
`3cc3fc2` 变为 `5df5547`，原 82 个文件的哈希一致。本轮在该历史上继续，新增的
主机测试、依赖与文档变更纳入 87 个文件哈希；未回退历史，未提交或推送。

## 安全审计发现及处理

完整门禁发现 [RUSTSEC-2026-0387](https://github.com/RustCrypto/crypto-bigint/pull/1365)：
原 `crypto-bigint 0.7.5` 存在常量时间运算问题。更新到 0.7.6，并按发布包的实际
依赖声明更新 `ctutils` 到 0.4.5。四个工作区均同步锁文件，不忽略该公告。

TUI 原 P-256 0.13 还引入受影响的 `crypto-bigint 0.5.5`。TUI 0.3.14 使用项目
已有的 P-256 0.14，采用该库的系统随机数生成入口及 SEC1 API。未改变备份通道的
坐标或原始 ECDH x 坐标编码；RFC 5903 §8.1 向量、无效点拒绝及新生成坐标的对等
共享值测试通过。未增加自定义标量采样算法，也未更改 Embassy 或工具链 pin。

发布包校验和及全部解包文件一致；新增两条标明自动化审阅的 cargo-vet 差异记录，
保留原豁免基线。审阅及软件测试不构成外部密码学审计或 RP2350 常量时间执行证明。
旧 Kani 受限项保持 pending，没有因依赖更新而抬高证明等级。

## 软件验证

完整门禁在 Nix 中执行 `HOST_TARGET=x86_64-unknown-linux-gnu ./scripts/check.sh`，
退出 0，末行 `ALL CHECKS PASSED`。脚本 3807 项、CLI 340 项、互操作 46 项测试
全部通过；门禁完成至部署前的 87 个文件哈希保持一致，候选原始 ELF 与门禁构建逐字节一致。
门禁日志 SHA-256：`17cf79d5a566dfb2e8a4d9b9ab8f799ac8a57fd242d36c2deab587cb8563ba4a`。

- 更新依赖后的两项现有触摸安全 Kani harness 通过。
- 57 项诊断工作流及轨迹解析模拟测试通过，其中候选工作流 40 项、解析器 17 项。
- 上游 `crypto-bigint` 全特性单元、属性及文档测试 901 项通过。
- 上游 `ctutils` 全特性测试 206 项通过，两个原有文档示例忽略。
- TUI 53 项测试、严格 clippy、主工作区与 TUI 的安全审计、cargo-vet 均通过。

此前门禁通过 3807 项脚本测试后，发现新增 CLI 连接站点未登记。已补齐
`fingerprint_trace` 的唯一设备连接分类及真实 argparse 入口测试；340 项 CLI 测试
通过，之后重新执行当前完整门禁。未降低测试下限或跳过失败项。

测量配置增加的 40 个矩阵单元保持 gap；既有证据等级不变。11 个证据包仅同步
当前配置总数，历史证明日志保留。生成页、门禁行数及引用检查保持一致。

中断期间新增跟踪的记录使手写文档从 104 增至 108，已将扫描下限由 52 提高到 54。
相关 133 项测试通过；已保留原门禁断言失败与受控停止的日志，再执行完整门禁。

临时构建曾因磁盘耗尽退出 101。已清理可重建的增量及旧变异编译缓存，
候选镜像哈希、源码与日志保留并重新核对，之后重跑完整门禁。

## 测量保护的故障注入

以下注入在 `09EF` 阶段执行，本轮对应守卫及 `check.sh` 调用行未变；当前未过滤
完整门禁另行验证。前两项执行原 `pytest (gate scripts)` 行，以 `-k` 聚焦对应保护，
各自未变异基线退出 0；最后一项执行原 `test (host)` 行。

| 临时副本缺陷 | 门禁行退出码 | 已核对的失败方向 |
| --- | ---: | --- |
| 删除测量镜像打包拒绝条件 | 1 | 镜像被错误接受，测试要求拒绝；返回 0 而非 1 |
| 删除诊断镜像构建行 | 1 | 保护测试要求该行存在，实际找不到 |
| 每页观察数由 2 改成 3 | 101 | 页与状态字超过单个 CTAPHID 初始报告容量，编译期断言拒绝 |

结果（本地文件：`target/held-trace-validation-09ef-2026-10-24/mutations.json`）和
执行脚本（本地文件：`target/held-trace-validation-09ef-2026-10-24/mutation-runner.py`）保留原调用行及筛选条件。

## 固定镜像与资源

UF2 SHA-256：`cffcff4da1f6234408f37076cfec0b2390cf9a8496e67b8e820ad084a4d34cf5`。
标志 `0x2A`，包含 fingerprint、strict-config 与 fingerprint-touch-trace。
实际记录缓冲符号 2064 字节，无新增测量堆分配或第一方 unsafe。栈 177824 字节，
门禁下限 172032 字节。UF2 共 3329 块，范围 `0x10000000..0x100D0100`；
各块载荷与 ELF LOAD 段及零填充对应，均在代码区内。镜像未签名，无 rollback epoch。

代码区为 `0..1 MiB`；存储区 `1..2 MiB`，NSBOOT 读写均拒绝。KVMAIN 896 KiB，
计数区 128 KiB，保持原布局及持久记录格式。

## 实机部署与结果

用户明确回复“授权刷写 09F0 并采样”，绑定本 SHA 与 ROM 序列号
`2E64C924B24D1A5E`；授权写入本地 `SESSION.json` 后才启动本机流程。先只读保存
新鲜 `09EE` GetInfo，再核对 ROM、secure-boot 0、分区围栏和旧 `09EE` 镜像。
受限工具执行 `load -p 0 -v`，随后原版 picotool 独立 verify，均退出 0，再重启应用。
没有清库、PIN 命令、签名或 OTP 写入；存储区仍为 NSBOOT 禁止读写。

应用在原 USB 端口 `1-2.1`、序列号 `rs-key-0001` 以 `09f0` 出现。前后
GetInfo 的 `clientPin` 均为 false；这仅比较报告的 PIN 配置状态，不验证持久内容。

| 检查 | 结果 | CTAP 状态 |
| --- | --- | --- |
| 普通触摸 1/2 | 通过，响应约 2.712 秒 | `00` |
| 普通触摸 2/2 | 通过，响应约 2.475 秒 | `00` |
| 窗口前长按，松手 7 秒后重新触摸 | 失败，响应约 30.000 秒 | `2F` |
| 长按后的独立新请求 | 失败，响应约 30.000 秒 | `2F` |
| BOOTSEL 备用确认 | 通过，响应约 4.159 秒 | `00` |
| 主机取消 | 通过 | `2D` |
| 不触摸的正常超时对照 | 通过 | `2F` |

两次失败均收到 UPNEEDED 和相同 CID 的原始终态报告；用户在本机弹窗确认操作顺序
符合提示、没有按 BOOTSEL 且已松手。`diagnostic.status=0` 只表示采样完成，失败项
没有改记通过。31 次适配器快照均为 Off、UART/驱动故障 none。

## 轨迹定位及边界

11 组原始分页全部离线重放通过，解码与保存结果一致；各组记录为一致的前缀，
最终序号 `0..62`，小于 128 条容量，没有覆盖。七个窗口均从 Ready、低电平、
Armed 开始。普通触摸的两个窗口各有一个上升边沿和一次消费；两个失败窗口没有
Edge 或 Taken，约 30001 毫秒后关闭。

以下时间为设备启动后的毫秒，不是主机时钟：

| 时间 | 记录 |
| ---: | --- |
| 60342 | 窗口外上升边沿，High，确认 Closed |
| 63449 | 约 3.107 秒后下降，Pending，随即进入 Running 维护 |
| 63678 | 维护结束时观察到高电平，进入 Settling |
| 66776 | 下降后返回 Ready、低电平 |
| 72059–102060 | 长按后请求窗口 24，Armed；无 Edge、Taken，最终超时 |
| 124260–154261 | 独立新窗口 32，Armed；无 Edge、Taken，最终超时 |

长按准备时已出现脉冲、下降和维护；到“仍保持触摸、尚未打开请求”的采样点，
现场已是 Ready／低／Off。结合用户操作确认，不能将 TOUCHOUT 下降直接解释为
手指已离开。失败轨迹也没有显示控制器停在 High、Settling 或 Disabled；
当前证据将后续定位集中到触摸输出及 GPIO 观察路径。

没有记录到边沿不证明物理线上没有跳变。IRQ 可能合并边沿，记录可能晚于状态改变，
现场电平只是采样瞬间。

追加只读采样未重启或刷写设备，仍为同一端口、同一次 `09f0` 枚举。先静置 5 秒，
再做两次 12 秒保持，各次之后松手 7 秒；用户在弹窗确认实际保持顺序。

| 实时采样段 | 样本数 | 电平及阶段 |
| --- | ---: | --- |
| 初始静置 | 100 | 全部 Low／Ready |
| 第一次保持 12 秒 | 231 | 全部 Low／Ready |
| 第一次松手后 | 129 | 全部 Low／Ready |
| 第二次保持 12 秒 | 228 | 全部 Low／Ready |
| 第二次松手后 | 126 | 全部 Low／Ready |

共 814 个样本，保持触摸段共 459 个。原始页逐条重放与保存字段一致，序号始终为
63，末次完整轨迹与首轮的 63 条观察一致。采样目标间隔 50 毫秒，实际最大读间隔
约 0.728 秒；不能据此排除更短的物理跳变。末次适配器快照为 Off、无故障。
采样完成退出 0，原始数据另存于 `live-gpio-01/`，首轮 39 份证据哈希保持一致。

后续应核对 TOUCHOUT 的脉冲及重新触发条件，并结合模块端与 GP2 波形、V_TOUCH／VCC
实测，再决定维护触发条件；不能仅凭延时或放宽确认规则宣称修复。

未验证实际 UP／UV 标志、指纹匹配、模板及凭据内容保持、新版 TUI 实机备份、
长期静置和不同供电。软件观察不能替代示波器波形或电压测量。默认安装仍仅离线校验，
已有操作记录时停止，不自动重复刷写。

## 停止检查点

用户要求“先记录一下工作结论 changelog，然后停下”。本轮已将上述结果加入
`CHANGELOG.md` 的 Unreleased，保存 `SESSION.json` 与恢复记录，并停止后续诊断。
设备保留已验证的 `09F0`，没有启动新的刷写、固件修改或波形测量。

完整门禁通过后，受其 87 文件清单约束的改动仅新增上述 CHANGELOG 实测记录；
固件与主机实现未改。结果记录的本地链接、声明、行数及补丁格式另行检查；
没有在停止请求后重跑完整门禁。门禁检查点中的硬件状态描述为部署前快照，
当前实机状态以 `SESSION.json` 及本记录为准。

全量 CHANGELOG 链接检查另外报告历史条目的
`guides/otp.md#challenge-response-from-software` 不存在；该链接在本轮前的 HEAD 中
已存在，未作为本轮故障修复扩展修改。新增记录链接和本结果文档单独核对。

## 本机证据

- 完整门禁（本地文件：`target/held-trace-validation-09f0-2026-10-24/gate.log`）
- 源码及退出码检查点（本地文件：`target/held-trace-validation-09f0-2026-10-24/checkpoint.json`）
- Kani（本地文件：`target/held-trace-validation-09f0-2026-10-24/kani.log`）
- 工作流测试（本地文件：`target/held-trace-validation-09f0-2026-10-24/workflow-final.log`）
- 依赖源码与测试证据（本地文件：`target/crypto-bigint-security-update-2026-10-24/review-evidence.json`）
- 供应链检查（本地文件：`target/crypto-bigint-security-update-2026-10-24/supply-chain-validation-final.log`）
- 镜像清单（本地文件：`target/held-touch-trace-candidate-09f0-2026-10-24/manifest.json`）
- 部署与采样说明（本地文件：`target/held-touch-trace-candidate-09f0-2026-10-24/README.md`）
- 写入与独立读回（本地文件：`target/held-touch-trace-candidate-09f0-2026-10-24/install-events.log`）
- 实机采样日志（本地文件：`target/held-touch-trace-candidate-09f0-2026-10-24/acceptance.log`）
- 分项结果（本地文件：`target/held-touch-trace-candidate-09f0-2026-10-24/diagnostic-result.json`）
- 原始分页重放及窗口分析（本地文件：`target/held-touch-trace-candidate-09f0-2026-10-24/hardware-analysis.json`）
- 追加实时 GPIO 样本（本地文件：`target/held-touch-trace-candidate-09f0-2026-10-24/live-gpio-01/samples.jsonl`）
- 追加采样结果（本地文件：`target/held-touch-trace-candidate-09f0-2026-10-24/live-gpio-01/result.json`）

`target/` 下原始证据为本机忽略文件，未表示已加入版本控制。
