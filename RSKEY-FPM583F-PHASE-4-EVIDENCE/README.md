<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# 第四阶段验收证据

2026-10-12。标准生物管理、持久恢复、客户端回归、四项真实 gate 反向验证和目标资源测量已完成；恢复后的原始完整 gate 退出 0，离线软件范围已验收。本阶段起点 HEAD 为 `7bece45dd2f33de8b6f7aaa027d1a1d6ceb147fa`。实现与硬件边界见[执行记录](../RSKEY-FPM583F-PHASE-4-RECORD.md)。

## 标准客户端

固定 python-fido2 2.2.1，BioEnrollment 使用实际 `FPBioEnrollment` API，经生产 FIDO 分派、目录、PIN/BE 认证和密码学处理。仅模块观察由脚本提供，独立库存文件在最终保存时更新。管理夹具使用目标 896KiB main / 128KiB counter，store 连同头部为 1,048,592B；没有实体设备枚举。

```text
21 passed in 22.53s
42 passed in 37.97s
24 passed in 7.92s
```

分别为[生物管理](../tests/bio_client/test_management.py)、第三阶段指纹和第二阶段基线矩阵，无静默 skip。覆盖 HID/CCID、PIN 协议 1/2、标准管理、UTF-8 最大名称、满库完整列举、质量反馈、取消/断开、保存结果未知、删除失败与槽位复用、最后模板 PIN 回退及 Reset 恢复。正常模板管理和重启后的认证均由原注册公钥验签。

## 持久故障与入口

[主机故障矩阵](../crates/rsk-fido/src/bio_enrollment_tests.rs)直接驱动 Fs 的 Cut、ProbeStuck 和提交结果未知后端，覆盖预留、Saved/Authorized 提交、删除停用/收尾、命名和 Reset 每个实际持久边界。另覆盖坏格式、缺失预算、库存变化、动态文件数耗尽、token 到期、来源断开及正常软锁。重开后的授权集合按已落地记录决定，读错或不完整扫描不解释为空库。

[Management 别名](../crates/rsk-device/src/ccid_bio_tests.rs)和[屏幕入口](../crates/rsk-display/src/pin_tests.rs)均驱动实际清理入口。下述反向验证确认新增测试也会经原始 gate 拒绝对应缺陷。

## 反向验证

每项临时变异均运行未经修改的 `scripts/check.sh`；实际 `test (host)` 行退出 101，格式与 lint 未先阻断。每次结束后恢复原始源码字节，并核对 SHA256；最终完整 gate 已从头验证恢复后的正常代码并退出 0。

| 临时变异 | 实际失败方向 | host 行结果 |
| --- | --- | --- |
| Directory 把 Saved 也当作已授权 | 保存而未完成授权提交的项应无配置，实际 `Ok(Some(1))`，期望 `Ok(None)`；未知提交与最终保存故障也错误授权 | FIDO 727 通过、3 失败、1 个原有忽略项 |
| 撤去单槽位的 Authorized 检查 | 另一槽位已授权时，隔离槽位实际获得 `Ok(true)`，期望 `Err(UvInvalid)`；正常代码逐项拒绝 Pending/Saved/Deleting，并允许原授权槽位 | FIDO 730 通过、1 失败、1 个原有忽略项 |
| 撤去屏幕清理前的生物状态检查 | 已确认的屏幕擦除应拒绝保留的生物隔离，实际执行成功 | Display 131 通过、1 失败 |
| 撤去指纹接入时 Management 禁用调用 | 清理别名应答 `INS_NOT_SUPPORTED=0x6D00`，实际 `SW_OK=0x9000` | Device 88 通过、1 失败 |

关键失败原文：

```text
final_authorization_requires_the_last_commit_at_every_cut_boundary
  left: Ok(Some(1))
 right: Ok(None)

an_isolated_slot_never_verifies_beside_an_authorized_template
  left: Ok(true)
 right: Err(UvInvalid)

a_display_build_with_retained_biometric_isolation_cannot_factory_wipe
retained biometric isolation must refuse even a confirmed display wipe

fingerprint_management_aliases_never_acknowledge_or_request_presence
  left: Sw(36864)
 right: Sw(27904)
```

最后恢复的源文件身份：

| 文件 | SHA256 |
| --- | --- |
| `crates/rsk-fido/src/bio_directory.rs` | `7396d68bb12c0ef9bce281bb4dfccdcde28b8948429b893bc39216c65d14dad8` |
| `crates/rsk-display/src/pin.rs` | `da591798a32e524a3ae3919c30373c0a7fba925b3537c29b8e6137622d4b6542` |
| `crates/rsk-device/src/ccid.rs` | `2cdb38ce699e01fa1361d399626eaba1d7bf1ca529f27dcbd49facd8787c8800` |

目录提交与测试预置辅助函数采用明确名称，避免静态所有者审计与既有 journal `append` 或通用 `new` 同名而错误关联。新增 token 来源标记登记为 alpha 不观察的 stutter，BE 管理授权登记为原 mc/ga/cm/acfg 模型范围外；既有指纹列的 37 项 gap 不因此关闭。更新引用、当前审计计数和生成页面，不改变原 TLA 性质、依赖或 gate。

## 2MB 资源

两种 raw ELF 均为 `FLASH_SIZE=2M KVMAIN=896K`、无 display/no-touch。实际链接符号确认代码区 1MiB、main 896KiB、counter 128KiB；各有内容的 Flash LOAD 段均落在代码区内。

| 项目 | 第三阶段指纹 | 第四阶段默认 | 第四阶段指纹 |
| --- | --- | --- | --- |
| GNU size 的 text（含 RAM 初始化数据） | 823520B | 832284B | 834556B |
| `.data` | 7872B | 7872B | 7872B |
| `.bss` | 335852B | 335468B | 335948B |
| 静态 RAM | 343724B | 343340B | 343820B |
| Core0 栈区域 | 180544B | 180928B | 180448B（176.21875KiB） |

本阶段指纹配置比第三阶段增加 text 11036B、静态 RAM 96B；栈仍比 168KiB 下限多 8416B。静态 RAM 取逐节 `.data + .bss + .uninit`，不采用 GNU size 总表的 data 分类：该 ELF 的可执行 `.data` 被总表计入 text，但仍占用 RAM。没有新增 heap 或 Core1 栈；真实最坏栈和板上时序未测。

## 最终完整 gate

任务中断后重新核对进程、日志和源码。中断前的一轮退出 1，停于删除操作审计登记；它不是完整通过的证据。后续脚本行又定位到 7 项旧计数断言和复现自检的管道误报；计数跟随实际树更新，gate 门槛不变。复现脚本读取完整拒绝说明，已有修复前失败、修复后通过的实际自检证据及 232 项相关审计回归。删除登记、当前 token 所有者、生成页面及引用已校准，四项变异恢复后从头执行原始完整 gate。最终正常代码从头运行退出 0，完整原件末行为 `ALL CHECKS PASSED`，gitleaks 无泄漏；源码身份逐项保持一致。

```text
3714 passed in 697.76s (0:11:37)
312 passed in 0.70s
45 passed in 0.03s
ALL CHECKS PASSED
```

FIDO host 731 通过、1 个原有忽略项；Device 89、Display 132、USB 63、模拟器 122 项通过。

## 复现

从仓库根目录使用固定 Nix 环境：

```sh
export NIX_CONFIG='experimental-features = nix-command flakes'
nix develop --no-update-lock-file -c ./scripts/bio-client.sh
nix develop --no-update-lock-file -c ./scripts/fingerprint-client.sh
nix develop --no-update-lock-file -c ./scripts/standard-client.sh
nix develop --no-update-lock-file -c env HOST_TARGET=x86_64-unknown-linux-gnu \
  CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true ./scripts/check.sh
nix develop --no-update-lock-file -c env FLASH_SIZE=2M KVMAIN=896K \
  cargo build --release -p firmware --frozen --offline
nix develop --no-update-lock-file -c env FLASH_SIZE=2M KVMAIN=896K \
  cargo build --release -p firmware --features fingerprint --frozen --offline
```

## 本地原件

`raw/` 保存完整日志、临时变异程序、源码恢复身份和测量程序，`build/` 保存 raw ELF；两者被精确忽略规则排除。源码、测试、本摘要与记录可随仓库保留。必要原件身份如下：

| 原件（相对 `raw/`） | SHA256 |
| --- | --- |
| `bio-client.log` | `47c464c5818e80160f1b4295689376bab7bac4140874300cac082c0516b01bbf` |
| `fingerprint-client.log` | `026626f444cbb393536718d8ea88449ef80a7a475ec1e4a13be9ee1cfca326df` |
| `standard-client.log` | `8c74c9f221b971f6682f69c8c9b4c16ccdd827fb4f00064071c7cc1eae80aea4` |
| `gate-mutation-saved.log` | `ff4a594ed7c520b5f72b60e3dcbac24f5b447a56885d708ecbf310e465265cf4` |
| `gate-mutation-binding.log` | `49d18f0a71eebdfd60b06312ed362fc2e4d5d8fcc43aaaef3975fe47ddf04a9a` |
| `gate-mutation-display.log` | `91c83867036630480401f4ff77ae0e1db5f2b4bda87c3d57d8920b8076994b52` |
| `gate-final.log` | `b501ea8190ee5995341c8ccbfe60f12782811e86678fb48df85aa89af3441bfa` |
| `gate-mutation-management.log` | `be03107ed44c408dfed1d5e58f676309ea4fb0e4ac7e88a22802a85e4535af3e` |

默认 ELF SHA256 为 `3ff34931cc033a7b23d32f2ae952022b77541432b3fcd98397ee76cbe7b04f7e`，指纹 ELF 为 `f043f63a5596927febda97f546f333393cfbc26652ad2a5c9ae5085b0f6e43c2`。它们均未应用分区表，只供资源审阅；完整 gate 会覆盖 target 下固件为 no-touch 测试镜像。

真实 583F 编解码、模块响应可信性、实物断电、USB/浏览器及第二客户端互通仍由阶段五至七验收。当前 Bridge 尚不提供实际 583F 业务，本阶段不是可部署的指纹镜像。没有设备操作、生产签名、提交或推送。
