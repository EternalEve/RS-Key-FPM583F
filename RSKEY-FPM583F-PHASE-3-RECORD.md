<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# 第三阶段执行记录

2026-10-12。第三阶段的离线软件范围已按保留双入口方案验收；正常完整 gate、两项实际 gate 反向验证和恢复后的最终完整 gate 均已完成。起点为 `53757d0e869b9079feaaea6e931816b4ca51fd31`，启动时工作树干净；范围见[第三阶段计划](RSKEY-FPM583F-PHASE-3-PLAN.md)。仅进行了源码、模拟器、证明和离线编译工作，未操作设备、提交或推送。

## 实现与范围

- **3.0 双入口归属：**HID CBOR/MSG 使用当前 CID，CCID 使用保留值 `CHANNEL_NONE=0`。断言续取、RP/凭据枚举与 MSE 按来源处理；ICC reset 只结束 CCID 自己的流程。SELECT/INIT 不全局撤销 token，也不提供新的 PIN 预算。凭据、PIN/UV 预算及原有 token 权限/RP/认证规则仍由两入口共享。
- **3.1–3.3 授权：**独立 Sensor/Bindings 服务通过已借用的 Fs 查询授权槽位和代次，两入口使用同一服务。UP-only 与 PIN token 不启动匹配；直接 UV/UV token 只接受当前操作的已授权匹配。交互性匹配可以提供 UP，token 的缓存 UP 在发行后 30 s 到期，不随滚动使用延期；消费后不能再批准下一次断言。屏幕 PIN 保留原确认路径，不冒充生物管理能力。
- **3.4 重试：**`EF_UV_STATE=0x1092` 为 `[1, retries, !retries]` 三字节记录，预算 8 次。先持久扣减并回读，再启动模块；只有明确尚未匹配的输入超时退款。正确 PIN 或合法指纹恢复 UV 预算；指纹不清除 PIN 临时锁，PIN 耗尽使 UV 为 0。已授权绑定下缺失、损坏、未知版本和提交失败均拒绝授权。Reset 将该拒绝状态作为普通记录清理，未扩大既有五个 PIN gate 记录的定义。
- **3.5–3.6 执行：**操作号、来源和截止时间隔离旧结果；逻辑终止后仍需确认模块空闲并排空旧应答，2 s 收尾超时则不可用。输入与匹配均可取消，分别报告 UPNEEDED/PROCESSING。同 CID INIT 在旧任务结束后回复，抑制旧普通应答；其他初始命令返回 BUSY。TCP 半报告跨请求保存。CCID 仍使用同步交换及 T=1 时间扩展，Cancel APDU 的 ACK 不代表中断正在运行的交换。
- **目标桥：**`fingerprint` 默认关闭；开启后占用 UART0 GP0 TX / GP1 RX、57600bps、8N1、固定 TX 128B/RX 256B 缓冲。有界轮询固定 HAL 的异步 I/O，软件 flush 后单独检查线路 busy，不使用阻塞 FIFO 等待。正式绑定为 Unconfigured，尚不发送 583F 命令或提供日用指纹认证。无屏配置可编译，常规构建拒绝与 display 同开。
- **模拟器：**`fingerprint-fixture` 明确限定 loopback、`--touch`、独占合成 store，无 display/USB-IP。只替代传感器观察和测试绑定，不替代 FIDO 判断或密码学。持久化错误向上传递，并隔离失败 medium，防止未落盘的 RAM 回读被当作提交成功；未声明 fsync 或实机断电耐久性。

行为变更已将 `bcdDevice` 从 `0x09DA` 增至 `0x09DB`。没有新增 unsafe 或分配器；直接使用的 `embedded-io-async=0.7.0` 已在固定 HAL 的依赖图中，Cargo.lock 仅增加 firmware 的直接边。协议、用户指南、构建说明与 CHANGELOG 已同步。

## 验收矩阵

“通过”指本阶段的软件/模型或离线编译证据；实物边界单列，不以脚本成功替代实机结果。

| 必要用例 | 结果与证据层 |
| --- | --- |
| 无指纹双入口 Begin/Next | 通过：真实分派拒绝 HID→CCID、CCID→HID、CID A→B；原来源续取成功 |
| RP/凭据枚举与 MSE 来源 | 通过：两入口真实 CM Begin/Next；CBOR/MSG/CCID MSE 归属及失败消费回归 |
| 交替 PIN 错误、UV 耗尽与恢复 | 通过：共享预算、正常软件重启、两入口正确 PIN 恢复 |
| FIDO2/U2F、APDU 链与其他 applet | 通过：原基线、实际 CCID U2F、双向链及原组合层测试 |
| 两入口直接 UV、UV/PIN token | 通过：六条断言路径由原注册公钥验签 |
| 一个入口收尾时另一个入口启动 | 通过：模块不能重复启动，PIN 加按钮可继续合法认证 |
| 正确 token 跨入口使用 | 通过：双向 PIN/UV token 按权限、RP 和 MAC 使用，不引入通道禁令 |
| 无模板/不可用情况下 UP-only | 通过：未配置时合法 UP，无匹配调用，UV=0，预算不变 |
| 正常直接 UV | 通过：当前授权匹配，UP/UV 正确，无额外按钮 |
| UV token 注册与断言 | 通过：两入口完整 WebAuthn 注册/认证及 RP 验证，仅一次触指 |
| 有效 PIN token | 通过：不启动匹配，实际按钮确认，原公钥验签 |
| 错权限、RP、MAC 与不可重用 token | 通过：明确拒绝；权限负例有同一合法 token 的正控制 |
| 旧 UP、重复终态与持续接触 | 软件通过：发行期限、滚动使用、消费、旧操作结果及原按钮释放模型；真实 TOUCHOUT 持续接触未测 |
| 不匹配、未知/停用/代次变化 | 通过：无新授权/只 UP 降级；协议 1/2 与核心绑定检查 |
| 输入超时与匹配状态未知 | 通过：只有证据充分的未匹配超时退款，未知/通信失败消耗预算 |
| PIN 临时/永久锁与 UV 阻断 | 通过：独立 UV、共享 PIN 限制，指纹不解除 PIN 临时锁 |
| 扣减/恢复失败与损坏记录 | 通过：提交故障、存储重开、缺失/未知版本/多余字节均不放行 |
| 输入/匹配 CANCEL | 通过：实际 Keepalive 观察点触发，两阶段均终止原请求 |
| 迟到成功、重复终态与收尾 | 通过：操作模型与状态机证明，旧结果不能重开请求 |
| 错 CID/跨传输 CANCEL 与 INIT | 通过：合法操作不被其他入口取消；同 CID 重同步后旧应答不污染下一请求 |
| 缓冲压力、部分报告、错误/无响应 | 软件通过：有界 I/O 五项与实际 TCP 半报告；真实 UART FIFO/IRQ/XIP 未测 |
| 收尾期间 PIN、静默探测及后续断言 | 通过：不被模块收尾占死，静默探测不增加匹配或消费正式授权 |

## 检查和反向证据

实际输出摘录、原件身份与复现命令见[证据摘要](RSKEY-FPM583F-PHASE-3-EVIDENCE/README.md)；完整日志、临时变异程序及 raw ELF 仅作本地归档。

| 检查 | 实测结果 |
| --- | --- |
| 固定 python-fido2 2.2.1 | 指纹矩阵 42、原基线 24 通过，无静默 skip 或 PIN 回退；两入口 WebAuthn 和原 PIN 凭据的新认证路径由原公钥验签 |
| Host 回归与 lint | 设备 87、USB 63、模拟器 122；最终 gate 的 FIDO 707 通过、1 个原有忽略项；职责拆分后复杂度峰值 35 |
| Kani 0.67.0 | 两项新增操作生命周期 harness 通过，不含真实模块、目录耐久和 UART，未运行完整 tier |
| 配置及登记 | 指纹/无 LED lint、四种 UART 引脚冲突的正确拒绝、复现入口登记、引用/计数及相关回归通过 |
| 原始完整 gate | 正常代码、变异恢复后均从头运行退出 0；最终脚本 3713、主机工具 312、互通规则 45 项通过，gitleaks 无泄漏 |

两次临时变异均执行未修改的 `scripts/check.sh`，实际 `test (host)` 行退出 101；结束后恢复原始字节及 SHA256。不是只调用辅助函数的反向检查：

| 临时变异 | 实际失败及正确方向 | 公开摘录 |
| --- | --- | --- |
| 分派的 `expire_user_presence` 时间参数改为 0 | GetInfo 与滚动 token 有效性正控制仍成功，随后因 `expired fingerprint UP survived CTAP dispatch` 失败；直接到期函数测试仍成功 | [UP 到期](RSKEY-FPM583F-PHASE-3-EVIDENCE/README.md#expiry) |
| 撤回 CCID 的 `stb.channel = CHANNEL_NONE` | 跨入口续取应拒绝 `0x30`，实际成功 `0x00`；MSE 来源实际为 2、应为 0 | [请求归属](RSKEY-FPM583F-PHASE-3-EVIDENCE/README.md#source) |

归属缺陷在不含指纹改动的起点 HEAD 副本中也已复现；修复前真实 host 行报错方向一致。存储提交错误有独立修复前失败、修复后两项回归通过的证据。

形式引用按原内容迁移，token UP、PIN 副作用、U2F 确认及取消分支另行审阅。既有 TLA/trace 结论仍限原 PIN/屏幕上下文；新增指纹列的 37 个 P0 性质保持 gap，三个缺屏性质为 out-of-scope，留待阶段四至六补证据。UP 到期在不观察 `user_present` 的 tier-A 投影登记为 stutter，不据此宣称真实 UP 已获形式证明。

## 2MB 资源与后续交接

最终两种 raw ELF 均以 `FLASH_SIZE=2M KVMAIN=896K`、固定 Nix 环境编译，未启用 display/no-touch；资源输出与 ELF 的 SHA256 保存在本地归档。主 KV 896KiB、计数器 KV 128KiB、代码区 1MiB 均未改变。

| 项目 | 第一阶段 raw 基线 | 当前默认 | 当前指纹 |
| --- | --- | --- | --- |
| size 的 text（含 RAM 初始化数据） | 815948B | 821816B | 823520B |
| `.data` | 7872B | 7872B | 7872B |
| `.bss` | 335252B | 335372B | 335852B |
| 静态 RAM 合计 | 343124B | 343244B | 343724B |
| Core0 栈区域 | 181152B | 181024B | 180544B（176.3125KiB） |

指纹配置相对第一阶段增加静态 RAM 600B；计入对齐后栈区域缩小 608B，168KiB 下限之上仍有 8512B。未增加既有堆或 Core1 栈。新增 UV 验证状态为固定尺寸，UART 缓冲有界；未测真实最坏运行时栈或板上时序。

- **阶段四：**正式授权目录、标准 BioEnrollment、完整 GetInfo 生物声明，录入/删除/Reset 与全部清状态入口的持久恢复协调。不能把测试 slot 7 或脚本初始化带入产品。
- **阶段五：**按已指定手册实现真实 583F 报文，在本阶段 start/poll/finish/service_idle 及 Stream 契约内推进，不重新引入阻塞 FIFO 或把 ACK 当作成功。
- **阶段六：**确认实物版本、电气、GP2 TOUCHOUT 新鲜性、UART 错误/取消/收尾、IRQ/XIP、真实栈及 USB 响应；本阶段的模块/总线信任假设不构成硬件响应认证。
- **阶段七：**真实 USB、浏览器、第二客户端及完整生物管理互通。

`check.sh` 留下的是 no-touch 测试镜像；raw ELF 不带分区表。本记录未产出可直接部署的指纹固件，设备候选必须按目标配置重建并加分区表，设备操作仍需具体授权。
