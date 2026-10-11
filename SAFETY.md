# RP2350 / RS-Key / FPM583F 开发安全注意

2026-10-11 更新；保留 2026-10-10 的 Pico FIDO 历史记录。后续 AI 和开发者开始固件、FIDO/PIN/凭据、指纹或设备工作前必读；本文不是设备操作授权。

## 1. 操作规则

- 用户当次明确指令优先。阅读/改文件/编译不包含连接设备、烧录、复位、BOOTSEL、擦除、运行 RAM 固件或写入数据。
- 未授权的设备步骤继续做离线方案；同一设备、同一具体操作已获授权时沿用，不重复询问，也不扩大范围。
- 普通烧录不包含 OTP、安全启动/锁定、密钥撤销、反回滚或关闭恢复入口。永久设置须先形成具体位/页、能力损失及恢复条件的方案，再取得对应授权。
- 不默认“查询无写入”“新板无 OTP 风险”“刷旧 UF2 能恢复”。审查启动/迁移、工具连接与断开、测试夹具和最终镜像。
- 未知或半完成状态停止追加写入；超时/报错不证明前序 OTP 写入未发生，不自动重试、擦除或修复。
- 日志/聊天/版本库不输出 PIN、密钥、模板、凭据秘密、私密 OTP 或完整敏感 Flash；不以导出私钥/绕过读保护作为普通备份。
- 日用 passkey 板不做错误 PIN 耗尽、整套硬件 pytest、自动 provisioning 或掉电/复位故障注入。当前空板可作为专用测试目标，但实机测试须先审查具体动作并取得对应授权；空板说明不包含这些操作授权。

区分三类不可恢复后果：OTP 永久硬件状态、无备份的秘密/模板丢失、过压/短路等物理损坏。OTP 位只能 0→1；硬锁永久，运行时软锁可随相应复位解除，权限按 Secure/Non-secure/Bootloader 域分别判断。

## 2. 固定基线与证据限制

### 当前目标与开发状态（2026-10-12）

| 项目 | 当前记录及依据 |
| --- | --- |
| 数据状态 | 用户明确说明：当前为全空板，板内无必要数据；当前开发不要求保留既有凭据/PIN，不建立 Pico FIDO 凭据迁移承诺 |
| 板级目标 | 计划采用 RP2350-Zero-CM、2MB Flash；本轮未连接设备，实物型号、序列号、芯片修订版及实际容量未验证，不沿用历史设备作为当前身份 |
| 固件源码 | 业务基线为 `7c9a57836647c363a9f006da1892e886869afb61`；阶段 1–3 的离线软件范围已验收，实际状态见阶段记录；当前板内固件未验证 |
| 开发环境 | Linux x86_64 与固定 Nix 环境；无屏 2MB 基线、真实重建及静态布局已核对，阶段 1–3 原始完整 gate 已通过 |
| 主机产物 | 已保留未签名的 2MB PT ELF/UF2；UF2 全部写块仅在代码分区内，没有额外 KV/RAM/OTP 写块。ARM Secure 是镜像执行域，不表示芯片已启用安全启动；未部署到设备 |
| 离线签名授权 | 用户明确允许运行 check.sh 既有随机临时密钥/临时 ELF 测试；本次范围不含设备动作或生产签名材料，基线产物保持未签名 |
| 未确认 | OTP、页锁、安全启动、反回滚、调试与恢复入口状态；无必要数据不证明 OTP 空白或可恢复 |

空板说明只更新当前数据保留要求；未来写入的测试状态仍按相应授权处理，
正式凭据建立后须重新记录数据与账号恢复要求。指纹模块的模板库存另行确认，
不能由主控空板推断模块为空。阶段执行证据见[第一阶段](RSKEY-FPM583F-PHASE-1-RECORD.md)、[第二阶段](RSKEY-FPM583F-PHASE-2-RECORD.md)与[第三阶段记录](RSKEY-FPM583F-PHASE-3-RECORD.md)。

### Pico FIDO 历史基线（2026-10-10）

以下表格与第 3–5 节的 Pico FIDO/PicoKeys 源码、工具路径和副作用结论为历史依据，
不代表当前空板状态，也不直接代表本仓库 Rust RS-Key 的行为。历史源码路径采用相应项目的相对索引，不表示文件存在于本仓库。

| 项目 | 记录 |
| --- | --- |
| 设备 | Waveshare RP2350-Zero-CM，2MB Flash；已试用真实 passkey |
| 固件 / 源码 | 官方 Pico FIDO 8.0 / `6ffcb81191c7f4e44337e0eaecc788ba6db03b75` |
| PicoKeys SDK | `7c5f4d9b95d5c17531a45c74c673822242d4d376` |
| 历史状态响应 | `00 00 FF`：固件未识别其定义的安全启动/锁定状态与默认密钥槽 |
| 未确认 | 完整原始 OTP、全部页锁/调试/恢复入口、运行二进制与源码逐项一致性、当前 PIN 配置 |

原 UF2 和状态 JSON 都不是凭据备份。Flash 镜像也不能默认跨设备恢复。

安全状态判断（历史路径：`pico-fido/pico-keys-sdk/src/otp/otp_rp2350.c:150`） 同时要求启用位及匹配硬编码哈希的有效槽；其他启动密钥可导致 false，锁定判断也依赖前者。两个 false/槽号 FF 不证明 OTP 空白、调试未禁用或全部槽未撤销。

### RS-Key 第一阶段基线的静态核对

在第一阶段固定业务基线中，[启动代码](firmware/src/main.rs)设置 OTP 密钥读取入口，
并调用 `sw_lock_key_page()` 设置运行时 SW_LOCK；[OTP 实现](firmware/src/otp_keys.rs)
中的永久页锁与 `ROLLBACK_REQUIRED` 写入通过[rescue 平台接口](firmware/src/rescue_platform.rs)接入，启动处未直接调用这两个写入函数。

已核对上述永久锁写入的 rescue APDU 调用链、条件和读回，以及第一阶段基线
IMAGE_DEF/分区元数据；具体索引见第一阶段记录。启动还会扫描存储、迁移
密封记录、ensure_seed 和初始化 OpenPGP；Yubico-OTP applet 冷启动计数写在 Flash，
不等于硬件 OTP 熔丝写入。第一阶段基线镜像不含签名或 rollback/version 条目。

这些静态与主机文件证据不能替代实物 OTP、Boot ROM 及首次运行的准入审查，
也不证明启动无数据副作用。Pico FIDO 的自动 OTP 初始化结论不直接套用于 RS-Key。

## 3. Pico FIDO 历史：自动初始化、迁移与数据损失

| 路径 | 实际影响与约束 |
| --- | --- |
| 普通启动 OTP 初始化（历史路径：`pico-fido/pico-keys-sdk/src/main.c:171`） | 空区可写 MKEK=`0xE90`、DEVK=`0xE80`、row+32 chaff，并永久设置 page 58 权限及运行时软锁；无需启用 Secure Boot |
| OTP 迁移（历史路径：`pico-fido/pico-keys-sdk/src/otp/otp_rp2350.c:125`） | 旧 MKEK/DEVK=`0xEF0`/`0xED0` 迁到新行后旧区写全 FF 失效；旧固件可能再无法使用。数值为 OTP 行号，不是操作参数 |
| FIDO 文件初始化（历史路径：`pico-fido/src/fido/fido.c:393`） | Flash 的 EF_KEY_DEV/EF_KEY_DEV_ENC 都缺失时生成新凭据派生密钥；与 OTP DEVK/MKEK 不同，保留 OTP 不保证旧凭据可用 |
| 证明私钥格式迁移（历史路径：`pico-fido/pico-keys-sdk/src/main.c:179`）、RP 迁移（历史路径：`pico-fido/src/fido/fido.c:520`） | 启动/初始化可重写 Flash；升级需查新旧格式与失败恢复 |
| CTAP Reset（历史路径：`pico-fido/src/fido/cbor_reset.c:32`） | 清空共享数据文件引用，涉及派生密钥、PIN、计数器、largeBlob、Vault/证书；新 keydev 可使旧非驻留/U2F 凭据失效，不能解除 OTP |
| 备份导入（历史路径：`pico-fido/src/fido/cbor_vendor.c:148`）、AUT_ENABLE（历史路径：`pico-fido/src/fido/cbor_config.c:197`） | 覆盖/转移 keydev 并清空原记录；不能当作无损修复或生物启用接口 |
| 驻留凭据注册（历史路径：`pico-fido/src/fido/credential.c:668`） | 相同 RP/userId 会复用旧槽；测试必须隔离 RP 和用户标识 |

不盲改根密钥、OTP 行/空白检测/锁位/迁移条件、RNG、派生算法或存储布局，也不简单删除 otp_init。先用合成旧状态验证兼容与失败恢复。

flash_commit（历史路径：`pico-fido/pico-keys-sdk/src/fs/low_flash.c:248`） 只排队，扇区擦写（历史路径：`pico-fido/pico-keys-sdk/src/fs/low_flash.c:108`） 在后台完成；同步接口（历史路径：`pico-fido/pico-keys-sdk/src/fs/low_flash.c:261`） 还需核对线程/错误语义。成功响应、RAM 重读或固定等待不证明耐久提交；真实板不拔线验证。

目标存在需要保留的真实凭据/PIN/模板时，删除、覆盖或擦除前确认账号有实际可用的其他登录/恢复方式；网站保存 credential ID 不代表仍能登录。

## 4. Pico FIDO 历史：永久安全配置与构建

- Secure Boot 实现（历史路径：`pico-fido/pico-keys-sdk/src/otp/otp_rp2350.c:200`） 写公钥哈希、有效位和 CRIT1，即使 secure_lock=false 也永久；强制 Arm、禁用 RISC-V。仍有 USB 下载不等于能运行任意恢复镜像。
- 启用函数的公钥哈希是固定值，SECURE_BOOT_PKEY 不会自动替换它；实际信任槽、签名私钥、架构、撤销状态必须一致。丢失私钥影响后续签署。
- Secure Lock 额外撤销其他槽、禁用调试、设置故障检测并收紧 OTP page 1/2 的 Bootloader 权限；未直接禁用 USB。不能承诺“必关 USB”或“BOOTSEL 总能救”。
- 多次 OTP 写入没有撤销事务。原管理按钮确认只是软件条件；新固件可改变它，不能替代审查。
- OTP 还涉及启动路径、Flash 参数、分区签名、下载与页访问配置；按准确芯片修订版审核，不能仅搜索 Secure Boot/Lock。
- version.cmake（历史路径：`pico-fido/pico-keys-sdk/cmake/version.cmake:45`） 在 RP2350+SECURE_BOOT_PKEY 下设置 ROLLBACK 5。secured 芯片启动满足条件的镜像时，Boot ROM 可在应用运行前推进 OTP 最低版本；非 secured 芯片忽略相关元数据，设备密钥初始化不等于 secured。
- 签名/生成配置只是主机动作；load/run/upload/UF2 可能连同执行。传输前核对实际镜像反回滚元数据及恢复镜像版本，不把所有签名镜像都判成会烧 OTP。
- 分区模板（历史路径：`pico-fido/pico-keys-sdk/config/rp2350/pt.json`） 按约 4MB 描述，本板 2MB：检查实际 linker map、加载地址、数据/绑定区及运行时边界。UF2 文件大小不能证明范围安全。
- 发布脚本（历史路径：`pico-fido/build_pico_fido.sh:1`） 清理本地目录并含默认签名密钥路径；执行前审查删除目标、部署/生产钩子。主机签名成功不证明可安全烧录。

依据：[RP2350 数据手册](https://pip-assets.raspberrypi.com/categories/1214-rp2350/documents/RP-008373-DS-2-rp2350-datasheet.pdf?disposition=inline) §5.1.11、5.9.2.1、13.4/13.5/13.10；[安全白皮书](https://pip-assets.raspberrypi.com/categories/1260-security/documents/RP-009377-WP-1-Understanding%20RP2350_s%20security%20features.pdf)；[PicoKeys 反回滚](https://docs.picokeys.com/production/anti-rollback/)。

## 5. Pico FIDO 历史：查询、连接与测试的副作用

| 入口 | 必须检查 |
| --- | --- |
| GetInfo（历史路径：`pico-fido/src/fido/cbor_get_info.c:242`） | 状态文件长度异常时写新随机状态并 flash_commit |
| Rescue SELECT（历史路径：`pico-fido/pico-keys-sdk/src/rescue.c:71`）、扫描（历史路径：`pico-fido/pico-keys-sdk/src/fs/file.c:365`） | 可因初始/损坏标识进入 Flash 编程 |
| FIDO SELECT（历史路径：`pico-fido/src/fido/fido.c:83`）、Management SELECT（历史路径：`pico-fido/src/fido/management.c:37`） | 可初始化密钥、扫描或迁移记录；SELECT 不天然只读 |
| info 脚本（历史路径：`scripts/pico_fido_device.py:75`） | GetInfo、Rescue SELECT 及覆盖主机 JSON；名字不证明零修改 |
| 硬件 pytest 夹具（历史路径：`pico-fido/tests/conftest.py:400`） | resetdevice、设置 PIN、凭据写入；枚举（历史路径：`pico-fido/tests/conftest.py:88`） 会选首台设备/任意卡 |
| Vault live 测试（历史路径：`pico-fido/tests/pico-fido/test_080_vault.py:316`） | 创建/删除/导入；尝试默认 PIN 后 skip 仍可能扣重试，destructive 标记未覆盖全部副作用 |
| ENABLE_EMULATION 平台选择（历史路径：`pico-fido/pico-keys-sdk/picokeys_sdk_import.cmake:285`） | Windows/Linux 可创建 TPM 持久对象或删探测密钥，macOS 可写永久 Keychain/安全硬件；Docker/仿真不证明纯软件 |

离线测试优先 独立 CTest（历史路径：`pico-fido/CMakeLists.txt:233`） 的假根/假文件，显式排除真实平台后端、设备传输和真实持久目录，不静默回退。模拟验证成功不得链接进日用固件。

PCSC 在连接时明确 SCARD_LEAVE_CARD，随后无参数 disconnect；当前装饰器不支持向 disconnect 传 disposition，默认可能 UNPOWER。当前 CCID POWER_OFF 只变插槽状态/LED，不等同 MCU 物理断电。

按 AID 与完整 APDU 识别：Rescue READ（历史路径：`pico-fido/pico-keys-sdk/src/rescue.c:415`） 为 CLA=80、INS=1E、P1=03；Management 恢复出厂（历史路径：`pico-fido/src/fido/management.c:195`） 同 INS、CLA=00。核对 P2/长度/负载，SELECT 失败停止，不能换 CLA 盲试。

只读约束下不能写诊断固件、重启、执行初始化 RAM 程序或注册/签名/重置来补证据。报告实际命令与已知内部副作用；无法确认的 OTP/底层写入保持未确认。历史 `00 00 FF` 不证明命令历史或零内部写入。

## 6. 指纹模块与实机准入

[FPM583F V1.0.2](RSKEY-FPM583F-REFERENCES.md#fpm583f)：VCC/V_TOUCH 3.3V、峰值 100mA、1.0mm 6Pin。确认实物型号/固件、排线方向、TX/RX、供电能力及上电/掉电高阻要求；禁止 5V/RS232 误接、断电倒灌，不泛称所有 RP2350 引脚耐 5V。

按用户指定，以 [583F/383C 通用协议 V1.2.0](RSKEY-FPM583F-REFERENCES.md#fpm383c583f) 作为报文与命令依据，电气/容量仍用 583F 规格书。通用协议的 57600bps/8N1、两层校验和命令回显不提供密码学认证；文档矛盾见第一阶段记录第 7 节。六针 583F 未定义 RESET 引脚，不套用通用单路供电模组的 RESET/深睡唤醒接线；启动延时、TOUCHOUT 脉冲及真实离手另验。

录入/删除/清库/恢复模块设置/升级固件会改数据；查询不夹带这些操作或未知安全等级/密钥锁定写入。断线、超时、解析错、无模板、未知结果不得算 UV 成功；接指纹不自动启用永久锁定或重建根密钥。

实机操作前只需形成具体记录：

1. 目标身份、实际 Flash 容量、真实数据与账号备用恢复方式。
2. 源码/SDK、构建参数、最终哈希/签名、加载范围与分区；测试/初始化/自动部署路径已审查。
3. 启动 OTP/迁移/旧行失效、安全配置及 Boot ROM 反回滚的实际结论；未知项明确列出。
4. 凭据/PIN/模板影响、仍可用的更新/恢复入口和符合签名/版本/布局的恢复镜像。
5. 电气、稳定供电和提交完成判断；用户授权的具体动作与范围。
6. 实际结果与非秘密错误记录；失败停止，不追加 reset、擦除或 OTP 修复。

更换 SDK、启动代码、存储布局、签名策略、工具或硬件后更新本文；新增设备状态标注时间、版本、来源和局限。
