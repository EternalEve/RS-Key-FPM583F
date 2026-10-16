<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# 第一阶段执行记录

2026-10-11。状态：前置准备与第一阶段（1.1–1.8）完成。无屏 2MB 基线、真正重建、模拟器构建及实际布局核对通过；原始完整 gate 从头运行退出 0，日志明确显示 `ALL CHECKS PASSED`。

本记录对应[第一阶段计划](RSKEY-FPM583F-PHASE-1-PLAN.md)的 1.1–1.8。保留未经业务修改的基线；本轮只进行主机环境准备、构建、测试、静态审查和说明更新。本记录保留第一阶段当时的基线结论；后续软件结果见第二、三阶段记录。完整日志、PDF 副本和构建产物仅作[本地归档](RSKEY-FPM583F-REFERENCES.md#上传与本地保留)。

## 1. 当前目标、源码与依赖身份（1.1）

用户说明目标为全空板，无需保留既有凭据/PIN。计划采用 RP2350-Zero-CM、2MB Flash；本阶段未连接设备，实物型号、芯片修订、实际容量、OTP 和模块库存未验证。

- 业务基线：`7c9a57836647c363a9f006da1892e886869afb61`。
- 基线构建 HEAD：`3863440c8b4737bd110c06fb7bd95ca5de7f24c3`；最终复核 HEAD：`f1e09b70918df9d194ce98e813e19eb5d18bc25c`。其业务源码与锁文件均与业务基线一致，保留原产物身份。
- 第一阶段没有提交、推送或设备操作。

第一阶段 Linux 文件字节的 SHA256 已与业务基线逐一核对；以下是历史身份，不声明后续修改后的锁文件仍同字节：

| 文件 | SHA256 |
| --- | --- |
| Cargo.lock | `f46dc8fb06060a5c07116f27b523f51c95f02a71974043ab702e3215b328576b` |
| flake.lock | `329b2d37efcae49a4dec8a14d28a8257304d999f78f6cfdd4e219b7d944b281b` |
| tools/emu/Cargo.lock | `7fb1e8eba983b8eaeb8a509238fb9498f081725b9a57e4bf9748e05230f7adf0` |

根、emu、fuzz 的 Embassy Git 修订均为 `6c28443489ad5940ab8c1824c090b1f8a7233bf6`。各工作区保留 `third_party/sequential-storage` 8.0.0 本地补丁，没有替换成同版本 crates.io 包。

共享源码不等于全部依赖版本一致，现有锁定差异如下，未用 `cargo update` 对齐：

| 依赖 | 根工作区 | emu |
| --- | --- | --- |
| embassy-usb | 0.6.0，同一 Git 修订 | 0.6.0，同一 Git 修订 |
| embassy-rp | 0.10.0 | 不使用板级 HAL |
| usbd-hid / usbd-hid-macros | 0.10.0 | 0.10.1 |
| minicbor | 2.2.2 | 2.3.0 |
| zeroize | 1.8.2 | 1.9.0 |
| sequential-storage | 8.0.0，本地补丁 | 8.0.0，本地补丁 |

已有许可及第三方文件保持原样；项目按 AGPL-3.0-only。第一阶段没有新增依赖、unsafe 或固件行为，`bcdDevice=0x09DA` 未变。指纹候选仅作资料比较，未引入代码。

## 2. 固定环境与复现入口（1.2）

所有 Cargo 构建、检查与 ELF 工具在固定 Nix 环境执行；本次观察到 Nix 2.35.2、Rust/Cargo 1.96.0、Python 3.13.13、ARM GCC 15.2.1、flip-link 0.1.12、rust-lld 22.1.2、picotool v2.2.0-a4。固件目标为 `thumbv8m.main-none-eabihf`，主机目标为 `x86_64-unknown-linux-gnu`。gate 使用的 Node v24.21.0 来自主机 PATH，当前 flake 不固定它。

主要输入为 nixpkgs `331800de5053fcebacf6813adb5db9c9dca22a0c`、fenix `3a556b6fbd42412b6f3f0ea8d35959b1826f86ff`；完整输入以 `flake.lock` 为准。开发环境原有 python-fido2 2.1.1 未作为指定客户端验收，第二阶段另行隔离固定 2.2.1。

```sh
export NIX_CONFIG='experimental-features = nix-command flakes'
nix build --no-update-lock-file --max-jobs 1 --cores 2 \
  .#firmware-2mb -o result-phase1-2mb -L
nix build --no-update-lock-file --max-jobs 1 --cores 2 \
  --rebuild .#firmware-2mb -o result-phase1-2mb -L
nix develop --no-update-lock-file -c \
  cargo build --frozen --manifest-path tools/emu/Cargo.toml \
  --target x86_64-unknown-linux-gnu -j 2
nix develop --no-update-lock-file -c \
  env HOST_TARGET=x86_64-unknown-linux-gnu CARGO_BUILD_JOBS=2 \
  CARGO_NET_OFFLINE=true ./scripts/check.sh
```

根、emu、tui、fuzz 四个工作区的锁定依赖均已准备。cargo-deny 读取全平台依赖；补齐缓存需避开仓库默认 embedded target，不能只 fetch 固件/主机两个 target 后就认定完整：

```sh
nix develop -c env -u CARGO_BUILD_TARGET bash -c '
  set -euo pipefail
  task_repo="$(git rev-parse --show-toplevel)"
  cd /tmp
  for manifest in Cargo.toml tools/tui/Cargo.toml tools/emu/Cargo.toml fuzz/Cargo.toml; do
    cargo fetch --locked --manifest-path "$task_repo/$manifest"
  done
'
```

固件 Nix 构建使用 `--offline --frozen`，emu 使用 `--frozen`，完整 gate 设置 `CARGO_NET_OFFLINE=true`；未升级 pin。首次环境准备及安全数据库获取需要网络，不将整个流程描述为断网。

## 3. 板级配置与预留（1.3）

实际使用上游 `nix/firmware.nix` 的 `firmware-2mb`：`FLASH_SIZE=2M`、`KVMAIN=896K`，计数器区固定 128KiB；未启用 display、no-touch、bench、always-uv 或 strict-config。实际构建输出见 `build-script-output.log`。未发现意外的 BOARD、LED、presence 或假密钥环境覆盖。

| 项目 | 基线实际配置/板级依据 | 后续预留或验证 |
| --- | --- | --- |
| USB | `1209:0001`，RS-Key Security Key，`bcdDevice=0x09DA` | 保持已有身份；行为变化按仓库规则递增 |
| Flash | 原理图 U1 为 W25Q16JVUXIQ，2MB；构建与布局均为 2MB | 实物容量、布局及更新恢复另验 |
| LED | WS2812，GP16，RGB 顺序，1 颗；官方 C/Python 示例均使用 GP16/RGB | 实物颜色与供电待测；不套用 One 的 4MB 配置 |
| LED 电源 | 板载 LED 接 3V3，无独立电源控制 GPIO；`PK_LED_POWER_ENABLED=0` | 不任意指定未接线的 power GPIO |
| 物理确认 | 原始 BOOTSEL 路径，`PK_PRESENCE_IS_GPIO=0`；BOOT 按钮接 QSPI_SS | FPM 触碰是独立待验证输入；RUN 是复位，不用作用户确认 |
| UART | 芯片 GPIO 功能表确认 UART0 TX=GP0、RX=GP1；CM 引出这两个脚；通用协议 p9 为 57600bps、8N1 | 设计预留，当前未初始化 UART；3.3V 电气按 583F 规格书，实物配置另验 |
| TOUCH | GP2 已引出，可作候选输入，无已知默认占用 | 模块输出极性、脉冲、持续接触及新事件语义未知，尚不是接线结论 |
| LED/USB/随机 IRQ | PIO0 SM0、DMA_CH0、PIO0_IRQ_0、DMA_IRQ_0、USBCTRL_IRQ、TRNG_IRQ；高优先级执行器 SWI_IRQ1/P2 | 候选 UART0_IRQ 中断缓冲，首选无新增 DMA；步骤 3 决定并验证 |
| Core1 | 既有 RSA 工作，16KiB 栈 | 不占用 Core1 承担指纹业务；大小已计入静态 RAM |
| 电源 | CM 原理图 U3 ME6217C33M5G，VBUS→3V3；厂商标称 LDO 800mA | 模块 VCC/V_TOUCH、启动/峰值电流、USB 供电预算、掉电倒灌均未知，接线前核算 |

构建生成的 display/wake 常量即使有值，也不表示无屏基线占用对应引脚；其使用在 display feature 下。原理图按官方固定提交保存并视觉核对完整单页：
板级依据为[官方 CM 原理图](RSKEY-FPM583F-REFERENCES.md#rp2350-zero-cm)，固定来源与副本校验值见资料索引。

[RP2350 数据手册身份](RSKEY-FPM583F-REFERENCES.md#rp2350)见资料索引。已视觉核对 GPIO Table 3（印刷页 17）和 RP2350-E9（印刷页 1358–1359）：E9 影响 A2 输入漏电，A3 已修复；若实物为 A2，输入被悬空或需要下拉时不能依赖内部下拉，手册给出 3V3 下拉 ≤8.2kΩ 的条件。实物 stepping 未确认，原理图元件标签不是实测身份，也不能替代模块输出电气核对。

现有 `EF_PHY` 可在启动时覆写 LED/power/presence 引脚，使用 `AnyPin::steal`；步骤 3 引入 UART/TOUCH 时必须将预留引脚纳入所有运行时碰撞校验，不能只靠编译期默认配置。当前无屏基线与 GP0/1/2 设计预留没有发现既有冲突；供电和模块端仍未确认。

## 4. 入口及生命周期审查（1.4）

已读取将执行的 Nix 构建、shellHook、build.rs、打包入口及完整 `check.sh`，并核对其设备相关命令和测试替身。执行的是主机编译、文件工具、单元测试和 mock；没有运行 `tests/emu.py` 认证链或直接针对设备的 `tests/NN_*.py`。

| 接入位置 | 触发条件、已观察的副作用与后续约束 |
| --- | --- |
| `.cargo/config.toml` | runner 是 `picotool load -u -v -x -t elf`，固件禁止用 cargo run 验证构建 |
| `nix/firmware.nix`、`scripts/pt.sh` | offline/frozen 编译；从 ELF 符号取得边界，picotool partition create / uf2 convert 仅写主机文件，未加载设备 |
| `flake.nix` shellHook | 准备工具 PATH、库路径、cache 临时目录和 Git hook；清理该缓存中的旧临时文件，不执行硬件入口 |
| `scripts/check.sh` | 主机测试显式排除 firmware/rsk-wipe；固件和 wipe 只编译。唯一签名行使用随机临时密钥、临时 ELF/OTP JSON，退出清理；用户已单独授权此离线测试，未涉及生产密钥/设备 |
| `firmware/src/main.rs` 启动 | USB attach 前 Fs 扫描、密封迁移、ensure_seed、OpenPGP 初始化；有 OTP 根密钥时执行 at-rest lap；冷启动可在 Flash 中推进 Yubico-OTP applet 的使用计数，与硬件 OTP 熔丝不同。真实启动并非无 Flash 副作用 |
| `firmware/src/otp_keys.rs` | `sw_lock_key_page` 是运行时 Non-secure SW_LOCK，保留 Secure 读取；永久 page58/ROLLBACK_REQUIRED 写入为独立 apply 函数 |
| `firmware/src/rescue_platform.rs`、`crates/rsk-rescue/src/lib.rs` | 永久 OTP 写入调用集中在 rescue 平台；通过 CLA 0x80 / INS 0x1B 管理 APDU，校验 magic、密钥/安全状态、现有行及物理确认后才执行并读回；不是 main 启动处直接烧这两个锁 |
| `firmware/src/worker.rs`、`crates/rsk-device/src/ccid.rs` | Management 的 device-wide factory_wipe 及重启；共有 FID 保留/清理谓词。生物 Reset 必须协调全部入口，不只改 CTAP Reset |
| `crates/rsk-fido/src/reset.rs` | CTAP Reset 时间窗、物理确认、显式 FID 清理、保留机构证明对象、seed 重建和 journal；加入生物目录及模块后需新的持久恢复协调 |

当前打包 ELF 的 picotool 元数据为 IMAGE_DEF + 分区表，无签名、hash 或 rollback/version 条目；ARM Secure 表示执行域，不等于已签名或已启用芯片安全启动。只观察主机产物和源码调用链，不据此保证任意 OTP 状态的设备一定能启动。设备首次运行、Boot ROM/启动元数据与恢复入口仍须按 SAFETY 的实机准入条件核对。

## 5. 构建、重建与现有检查（1.5）

| 检查 | 第一阶段实际结果 |
| --- | --- |
| 固定环境及四工作区依赖准备 | 退出 0；全平台缓存补齐后原 cargo-deny 行通过 |
| 无屏 2MB 构建、PT 打包、UF2 转换 | 退出 0，逐块地址及 ELF 内容核对通过 |
| `nix build --rebuild` | 真正重新编译，Nix 输出一致性检查通过；PT ELF/UF2 前后哈希相同 |
| 捕获实际 memory.x / raw ELF | 仅增加归档复制；PT ELF/UF2 与原包逐一相同 |
| 模拟器独立主机构建 | 显式 host target 的 frozen 构建退出 0，尚未运行认证闭环 |
| 原始完整 gate | 从头运行退出 0，`ALL CHECKS PASSED`；脚本 3713、CLI 312、interop 45 项通过，gitleaks 无泄漏 |
| 手册样例核算及最终引用复核 | 11 组明确示例的长度/双层校验重算通过，88B 位图帧预算核对；最终原始完整 gate 再次通过 |

完整 gate 使用既定默认、16M、display、no-touch 和 wipe 构建矩阵；2MB 验收另依据本阶段的构建、强制重建及实际布局，不能从默认矩阵推断目标容量。保留的基线产物身份为：

| 本地原件 | SHA256 |
| --- | --- |
| 未签名 PT ELF | `5430da11d3d7f0ba351ceb1ddf6e3bbf36917f311d580bf827a8cfede990c7f3` |
| 未签名 UF2 | `6967a172bc892d400b74798b60380dab8606f7b22328f629f9673a104e7390a4` |

raw ELF、memory.x、完整日志与上述镜像保存在忽略的第一阶段证据目录；不作为源码交付物。`check.sh` 最终覆盖 target 下的 firmware 为 no-touch 测试配置，不能将其误认作此处的 2MB 基线；raw ELF 也不带分区表。

已有 RSA section attributes 和原策略允许的未维护/yanked 依赖警告保持原样，没有借升级 pin、删除警告或新增豁免恢复 gate。可重现结论仅针对当次固定输入与工具链，不能外推不同平台/编译器的字节一致性；文档向量核算不等于驱动或实物兼容验收。

## 6. 实际布局与增量门槛（1.6）

以第一阶段实际 memory.x、ELF LOAD、符号、分区表和 UF2 测量；完整工具输出留在本地归档。以下范围均为半开区间：

| 区域 | 实际范围/容量 | 基线占用与剩余 | 增量/复测依据 |
| --- | --- | --- | --- |
| Flash 代码 | 相对 `0x000000..0x100000`，1024KiB | PT ELF 加载高水位 816100B（796.973KiB），剩余 232476B；UF2 补齐后 816128B（797KiB），剩余 232448B（227KiB） | 指纹增量尚无法可靠估算；步骤 3/4/5 每次链接复测，不跨 KV 边界 |
| 主 KV | 相对 `0x100000..0x1e0000`，896KiB，224 个 4KiB sector | 不承载 ELF 段；没有读设备，实际已用容量未知 | 正常凭据、生物目录、pending 与 GC 共同使用，步骤 4 验证目标几何容量 |
| 计数器 KV | 相对 `0x1e0000..0x200000`，128KiB，32 个 sector | 不承载 ELF 段；实际已用容量未知 | 后续持久重试状态的写入/恢复需独立评估 |
| 链接 RAM | `0x20000000..0x20080000`，512KiB | `.data=7872B`、`.bss=335252B`、`.uninit=0`，合计 343124B；顶端剩余对齐 12B | 不是芯片全部 520KB；堆、Core1 栈、任务池均已计入 |
| Core0 栈区域 | `_stack_end=0x20000000`，`_stack_start=0x2002c3a0` | 181152B（176.906KiB）；高于现有 gate 的 168KiB 下限 9120B（8.906KiB） | 新增静态占用总上限为 9120B，仅是保持链接下限的天花板；步骤 3/6 测运行时最坏栈，不能用完该数后直接认定安全 |
| 既有堆 | 128KiB，BSS 的 HEAP_MEM | 已包含在上述静态 RAM，服务既有 RSA bigint | 不另扣、不扩大，不为指纹新建堆；上游 no-alloc 规则约束新增代码，已有例外见 docs/unsafe.md 的 heap init |
| Core1 栈 | 16KiB；所在 StaticCell 符号 16416B | 已包含在 BSS | 当前 RSA 使用；运行时未测 |

2026-10-17 M5 复核补注：上表堆容量和布局测量不变，但“服务既有 RSA bigint”
未列全用途；FIDO 的 ML-DSA 展开密钥也通过 `Box` 使用该堆。分配例外与待验证
边界现统一记录在[内存模型](docs/architecture.md#memory-model)，本补注没有新增实测。

代码高水位包含向量表/IMAGE_DEF（316B）、`.text`（757512B）、只读 LOAD（50260B，含对齐）、初始化 `.data` 的 Flash 副本（7872B）及 PT（140B）。debug section 不加载到设备，ELF 文件约 17MB 不能当作 Flash 占用。UF2 文件 1632256B、3188 个 512B 块，每块有效载荷 256B。

逐块验证 magic、`flags=0x2000`、family `0xe48bff59`（RP2350 ARM Secure）、声明数量、唯一连续索引/地址；范围 `0x10000000..0x100c7400` 全在代码分区。UF2 与每个 ELF Flash LOAD 逐字节相同，末尾仅 28B 零填充；没有额外 KV/RAM/OTP 或 absolute-address 写块。family 解释来自 [Pico SDK 2.2.0 官方 UF2 定义](https://github.com/raspberrypi/pico-sdk/blob/2.2.0/src/common/boot_uf2_headers/include/boot/uf2.h)。

实际 PT：代码 `0..0x100000` 可更新、family rp2350-arm-s；store `0x100000..0x200000` 为 data，Non-secure/NSBOOT 写入被拒绝。与 `__kvmain_start/end`、`__kvcnt_start/end` 及实际构建存储长度一致。PT 保护结论仅描述该文件内容，不证明已经部署到板上。

主要静态 RAM 符号供后续比较，不能再次加到静态总数：Fs 35764B、FIDO_STATE 5840B、worker EXCHANGE 15252B、main task pool 36272B、worker pool 39504B、CTAP pool 31248B、CCID pool 8568B、两组 RSA sieve 各 5392B。上游栈峰值注释约 114KiB 是原有依据，本轮没有实测复现。

| 指纹增量项 | 目前估算分类/预算方式 | 复测阶段 |
| --- | --- | --- |
| UART RX/TX 与解析 | 新手册已给出必要命令长度：库存位图应答为 88B（11B 链路头 + 77B 应用层）；这只是所选命令集合的帧上限依据，环形缓冲还须覆盖峰值及 Flash 停顿，不上传模板/图像 | 3 原型大小与等待模型；5 逐命令帧边界；6 溢出实测 |
| 模块操作状态 | 尚无实现大小；保持单个有界操作状态、取消代次与未知结果，不并存多套驱动/任务 | 3/5 `size_of` 与 ELF 增量 |
| 模板元数据/pending | 格式未定；100 槽已有 583F 规格书依据，实际容量/槽号范围未实测。优先从持久记录按需加载，不能先扩成常驻大数组 | 4 最大记录/页回收/失败恢复；5/6 容量与槽号核对 |
| BioEnrollment 请求响应 | 复用已有 maxMsgSize 7609 和 worker EXCHANGE，避免再复制一组最大 CTAP 缓冲；增加编码器状态仍需实测 | 4 主机边界测试、Flash/RAM 增量 |
| 新任务与栈/IRQ | 候选 IRQ 缓冲加同步 worker 有界轮询；新 future/task pool 和 IRQ 嵌套不能记作零开销 | 3 编译/模型；6 最坏栈/响应时延 |

全部新增静态项合计须低于 9120B 才能保持当前栈下限，并保留实际运行余量；这不是对最终全部指纹功能容量的承诺。主存储现有 `MAX_RESIDENT_CREDENTIALS=256`、`MAX_DYNAMIC_FILES=1280`、`MAX_VALUE_BYTES=4078` 是条目/单记录上限，不能据此推断 896KiB 一定容纳所有 applet 的最大集合。步骤 4 必须在 2MB 的 224/32 页几何下测试正常凭据、生物记录、更新日志和 GC 工作空间；默认 emu 的 352/32 页不能代替该容量验证。

## 7. 模块必要能力及候选资料（1.7）

电气、排线和标称容量依据 [583F 规格书 V1.0.2](RSKEY-FPM583F-REFERENCES.md#fpm583f)；命令与报文按用户指定的 [583F/383C 通用协议 V1.2.0](RSKEY-FPM583F-REFERENCES.md#fpm383c583f)。资料索引保留原件版本与 SHA256，PDF 不随源码上传。

文档版本不等于实物固件版本；实际模块型号/行为留待第六阶段核对。以下页码均为对应 PDF 的印刷页，已核对相关完整页面的图表与文字。

### 报文依据

通用协议 p9–12、p45 定义 57600bps、8N1；帧头为 `F1 1F E2 2E B6 6B A8 8A`，随后是 2B 应用层长度、1B 帧头校验和及应用层数据。长度只计算应用层；请求为 4B 校验密码、2B 命令、参数和 1B 校验，响应另含 4B 完成码。帧头校验覆盖帧头和长度，应用校验覆盖应用层除其末尾校验字节的全部内容；两者都取逐字节和的 8 位补数，使各自含校验的和模 256 为 0。

没有 EF01 的设备地址或包类型字段，也没有事务编号。响应回显命令，因此可拒绝错误命令回复，但不能仅靠回显区分连续两次相同命令。报文中的 4B 密码是明文校验字段，文档的“加密”表述不构成密码学保密或响应认证；可信模块/总线假设保持不变。

### 必要能力

| 必要能力 | 已有参考与当前判定 | 缺口、下一动作/阶段 |
| --- | --- | --- |
| 电气/启动/忙/休眠 | 规格书 p10–14：VCC/V_TOUCH 3.3V、峰值 100mA、6Pin、断电前 UART 高阻；p16–17：启动 `0x55`、建议 150ms。通用 p34–35：`02 0C` 普通/深度休眠 | 启动延时有矛盾；583F 六针定义没有 RESET 脚，不套用通用深睡唤醒/单路复位接线。供电与实际就绪步骤 6 验证 |
| UART 帧、长度和错误 | 通用 p9–12、p40–41、p45 已给格式与错误码；库存位图 p26 为 88B 完整应答 | 可准备有界 codec；逐命令区分成功/错误长度，不把 88B 当任意功能的总上限；步骤 5 |
| 新触碰事件 | 规格书 p14：休眠唤醒高脉冲；通用 `01 35` p26–27 查询手指在位，STATE=0/1 | 查询状态不能直接替代新事件；验证持续接触、离手及重装中断，步骤 3 模型、6 实测 |
| 当前新鲜匹配与槽号 | `01 21` 发起匹配、`01 22` 查询，p21–22、p43–44；返回结果、分数、槽号，结果=1 才是匹配 | 开始 ACK/完成码=0 均不单独提供 UV；实际新采样边界、忙查询和迟到隔离步骤 5–6 验证 |
| 录入进度与最终保存 | `01 11/12` 逐次录入/查询，p16–18、p42–43；`01 13/14` 指定槽保存/查询，p18–19。`01 18` p21 是自主采样并保存的自动录入 | 首版优先手动推进；PROC 是百分比，不直接等同有效样本数。最终保存查询与掉电耐久分别验证；步骤 4–6 |
| 模板目录/容量/槽复用 | `01 33` 指定槽查询 p25–26，`01 34` 64B 占用位图 p26，`02 03` 已录入数量 p32–33；规格书标称 100 枚 | 通用位图有 512 位，不代表 583F 支持 512 槽；NUM 示例有疑点，真实槽界/计数/完整性步骤 5–6 核对，存在不等于授权 |
| 删除与清库 | `01 31/32` 删除/查询，p23–25、p44–45；CL_FLAG=0 定向、1 全库；另有同步 `01 36` p27–28 | 先耐久停用主控授权；查询完成后核对指定槽/全库，掉电恢复仍持久隔离；步骤 4–6 |
| 取消与收尾 | `01 15` p19 定义取消注册或匹配，强制中断错误 `0x15` 见 p40 | 适用范围不能外推到保存/删除；取消 ACK、原终态和排空分界仍需证明；步骤 3/5/6 |
| 写操作超时 | 保存 `01 14`、删除 `01 32` 有结果查询；忙码=4 建议延迟查询，p43/45 | 受总截止时间约束；查询成功只描述当前状态，结果丢失/重启保留 Unknown/pending，不照抄重发写操作建议；步骤 4/5 |
| 版本/身份/重复指纹 | `03 01` p37–38 返回 16B 模块 ID 示例，`03 03` p38 为心跳；重复错误 `0x0F` p18–19/40，策略查询 `02 FB` p36 | 未找到独立固件版本/唯一序列号查询；ID 不是响应认证。重复检查与自学习受策略影响，不自动改策略；步骤 5/6 |

### 手册矛盾与处理边界

| 项目 | 冲突位置 | 当前文档采用的依据及待确认项 |
| --- | --- | --- |
| 多字节顺序 | p10 写 little endian，同时明确例子为高字节在前；p39 密码字段表与完整帧也相反 | 常规字段按明确的高字节在前示例准备独立向量，不实现自动双端序兼容；重复 ID 等特殊字段单独核对 |
| 启动时间 | 通用 p9 约 200ms、p41 约 100ms；583F 规格书 p16–17 建议 150ms | 保留三处原始数值及适用来源；软件可准备有界启动窗口，实物就绪与启动字节先验再固定参数 |
| 录入终点与次数 | p16 写 PROC>100 并允许 6 次后增加；p17 完成示例为 100，p42 流程为 >=100；p21/35 次数范围 1–6 | 以明确的 100 完成示例准备测试，绝不因已经发满 6 次自动完成；有效样本预算、提前完成和失败继续按真实结果确定 |
| 响应结构 | p13 汇总将 `01 14` 数据长度写为 0，p18–19 成功结果含 2B ID；p22 忙错误有不含匹配数据的短响应 | 按具体命令详情与完整示例建立成功/错误结构；短忙帧不算损坏或成功，错误响应缺槽号不能产生匹配 |
| 库存与重复 ID | p26 NUM 示例为 `20 00`，与位图范围/583F 容量不相符；p18 重复 ID 文字指定反向顺序，示例零值无法区分 | 不从 NUM 示例推导容量或授权；占用查询可采用 `01 33`，重复 ID 顺序/位图 NUM 含义留待明确回复或实物确认 |

普通校验密码不加密总线，模块 ID 不证明真伪。手册 p20/p46 的 `01 16/17` 自学习更新会改变模板，不夹带在只需 UP 或诊断查询中；配置/密码/波特率命令也不用于自动探测兼容性。

### 候选复用边界

已读取固定提交的帧头定义，以下三项都采用 `0xEF01`，与通用 V1.2.0 不同；本次不引入代码或依赖：

| 候选 | 已观察的可复用点 | 适用性/许可限制 |
| --- | --- | --- |
| [r503 0.1.0 帧定义](https://github.com/embediver/R503-rs/blob/2175d7ca7e77da2a4e7e4b3ab20221afb6a5e8f7/r503/src/types.rs#L127) | Rust embedded-io-async，typed ConfirmationCode、SlotTable、SystemParameters | Apache-2.0 OR MIT；只参考类型/运行模型/测试，不直接复用 EF01 编解码或命令 |
| [a608_embedded 帧定义](https://github.com/NightSling/a608_embedded_hal/blob/e8ae902a469aa165a78a76389de5302c75232e21/src/lib.rs#L7) | Rust，同步 I/O/固定缓冲；当前匹配状态保存在对象中 | manifest 标识 MIT；只参考缓冲/测试，零进度循环、丢弃校验和及旧结果保留须另评估 |
| [FPM-C 帧定义](https://github.com/brianrho/FPM-C/blob/2255a7feab69ecf493bc743fd67e88db59463e4d/src/fpm.h#L61) | C；UART/时钟回调、解析状态参考 | 同一头文件声明 MIT；只参考状态/超时思路，不引入 EF01 协议与跨语言运行层 |

第五阶段按新手册做最小适配；如真实能力与第三、四阶段契约冲突，先暂停受影响实现，不能以旧协议字段补齐。

## 8. 后续接入索引、风险与阶段判定（1.8）

| 位置 | 后续涉及的调用方/职责 |
| --- | --- |
| `crates/rsk-sdk/src/presence.rs` | UserPresence 的通用确认和 collect_pin，当前 UV 对应可信屏幕 PIN；指纹只接 FIDO，不能把所有 applet 的确认改成匹配 |
| `crates/rsk-fido/src/clientpin.rs`、`state.rs` | UV/PIN token、权限和重试；当前 0x06/0x07 由 uv_available 控制、PERM_BE 未支持、builtin UV 依赖 EF_PIN；生物预算与 PIN 锁定须独立 |
| `crates/rsk-fido/src/getinfo.rs`、`makecredential.rs`、`getassertion.rs` | 能力声明及直接 UV/builtin_uv_step 的调用点；目前没有标准 BioEnrollment 分派 |
| `crates/rsk-device/src/ctap.rs`、`firmware/src/handler.rs` | 实际 AppletHandler/FIDO 状态、Fs/Rng/presence 共享借用；旧记录的 `firmware/src/ctap.rs` 路径不存在 |
| `firmware/src/worker.rs`、`presence.rs`、`main.rs` | 同步业务 worker、Keepalive/CANCEL、IRQ 输入和初始化；逻辑代次应拒收取消后的迟到结果 |
| `crates/rsk-device/src/ccid.rs`、`crates/rsk-fido/src/reset.rs`、`rsk-mgmt/rsk-vendor` | 全部清理入口及谓词；模块清理和授权目录一致恢复，不能只清一条 CTAP 路径 |
| `crates/rsk-fs`、`crates/rsk-store` | 主控版本化目录、pending、持久重试与隔离恢复；记录更新需保持既有凭据/密封兼容 |
| `tools/emu/src/device.rs`、`presence.rs`、`signals.rs`、`hid.rs`、`rng.rs`、`store.rs` | 复用实际 applet/存储；脚本模块/确认注入；OS 熵；CTAPHID reports 和待取消响应 |

第一阶段尚未实现执行桥或标准客户端认证；上述源码索引与第 6 节预算用于后续工作，不代表对应能力已通过。标准客户端与指纹软件闭环的后续结果分别见[第二阶段记录](RSKEY-FPM583F-PHASE-2-RECORD.md)及[第三阶段记录](RSKEY-FPM583F-PHASE-3-RECORD.md)。

| 未决边界 | 后续验证 |
| --- | --- |
| 最终 Flash/RAM 与最坏运行时栈 | 每阶段重新链接测量；板上栈、IRQ/XIP 与响应时延在阶段六验证 |
| 2MB 存储压力与恢复 | 按目标 224/32 页几何验证凭据、生物目录、日志及 GC，不外推默认 emu 的 352/32 页 |
| 手册矛盾与真实模块行为 | 阶段五准备逐命令独立向量及最小适配；阶段六核对电气、时序、取消与耐久 |
| 实物身份、OTP、启动及恢复 | 任何设备动作前按 SAFETY 单独核对和具体授权 |

第一阶段按 1.1–1.8 完成。工程步骤验收不等于产品实物里程碑完成；本阶段没有设备动作或生产签名。
