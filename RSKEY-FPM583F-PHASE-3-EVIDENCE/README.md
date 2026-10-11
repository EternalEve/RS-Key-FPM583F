<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# 第三阶段验收证据

2026-10-12。保留 HID/CCID 双入口的离线软件范围已验收，起点 HEAD 为 `53757d0e869b9079feaaea6e931816b4ca51fd31`。结果、资源和后续边界见[执行记录](../RSKEY-FPM583F-PHASE-3-RECORD.md)。以下摘录来自当次原件；完整日志、临时验证程序和构建对象留在本地。

## Clients

固定 python-fido2 2.2.1 的指纹双入口矩阵、原标准客户端基线依次为：

```text
42 passed in 38.33s
24 passed in 7.95s
```

无静默 skip 或 PIN 回退；两入口完整 WebAuthn 与原 PIN 凭据启用指纹后的认证路径均用原注册公钥验签。实现断言见[指纹客户端测试](../tests/fingerprint_client/test_authorization.py)；设备 87、USB 63、模拟器 122 项回归通过。

## Kani

固定 Kani 0.67.0 验证[两项操作生命周期 harness](../crates/rsk-fido/src/fingerprint_kani.rs)：当前活动操作才可接受结果；终态不能在确认排空前重开。

```text
Complete - 2 successfully verified harnesses, 0 failures, 2 total.
```

范围不含真实模块、目录耐久或 UART；没有运行完整 Kani tier。既有 TLA/trace 结论仍限原 PIN/屏幕上下文；新增指纹列的 37 个 P0 性质保持 gap、三个缺屏性质为 out-of-scope，不以登记数量代替证明。

## Mutations

两次变异均在测试工作树运行未修改的 `scripts/check.sh`，实际 `test (host)` 行退出 101；结束后原始字节与 SHA256 均恢复一致。

### Expiry

将分派的 `expire_user_presence` 时间参数改为 0。GetInfo 与 token 有效性正控制仍成功；[真实分派回归](../crates/rsk-fido/src/fingerprint_tests.rs)因应失效的 UP 仍有效而失败，直接到期函数测试仍成功：

```text
---- fingerprint::tests::ctap_dispatch_expires_fingerprint_presence_without_retiring_a_used_token stdout ----
expired fingerprint UP survived CTAP dispatch
test result: FAILED. 706 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.89s
```

### Source

撤回 CCID 的 `stb.channel = CHANNEL_NONE`。[CCID 续取回归](../crates/rsk-device/src/ccid_tests.rs)应拒绝跨入口续取（0x30），实际成功（0x00）；同次 MSE 回归实际来源为 2、应为 0：

```text
assertion `left == right` failed: CCID must refuse a continuation authorized on another transport
  left: 0
 right: 48
test result: FAILED. 85 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

此归属缺陷也已在不含指纹改动的起点 HEAD 副本中复现。存储提交错误的两项[模拟器回归](../tools/emu/src/store_tests.rs)有独立修复前失败、修复后通过证据，不将 RAM 回读冒充提交成功。

## Final gate

正常代码的完整 gate 先通过；两项变异恢复后又从头运行原始完整 gate，退出 0，gitleaks 无泄漏。恢复后的最终原件：

```text
3713 passed in 681.27s (0:11:21)
312 passed in 0.68s
45 passed in 0.04s
ALL CHECKS PASSED
```

FIDO host 行为 707 通过、1 个原有忽略项。构建/lint、四种 UART 引脚冲突的正确拒绝、复现入口登记、引用及计数回归均通过；中间单项日志不代表整次 gate 通过。

## Reproduction

从仓库根目录使用固定 Nix 环境；Linux host target 为 `x86_64-unknown-linux-gnu`：

```sh
export NIX_CONFIG='experimental-features = nix-command flakes'
nix develop --no-update-lock-file -c ./scripts/standard-client.sh
nix develop --no-update-lock-file -c ./scripts/fingerprint-client.sh
nix develop --no-update-lock-file -c env HOST_TARGET=x86_64-unknown-linux-gnu \
  CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true ./scripts/check.sh
nix develop --no-update-lock-file -c env FLASH_SIZE=2M KVMAIN=896K \
  cargo build --release -p firmware --frozen --offline
nix develop --no-update-lock-file -c env FLASH_SIZE=2M KVMAIN=896K \
  cargo build --release -p firmware --features fingerprint --frozen --offline
```

指纹夹具限 loopback、`--touch` 和独占合成 store，不枚举实体安全钥匙。正式绑定仍为 Unconfigured，尚不发送真实 583F 指令；编译通过不构成日用指纹或实物验收。

## Local archive

`raw/` 保存完整输出、临时变异程序、验收时历史快照和整理前文档；`build/` 保存两种 2MB raw ELF，均受精确忽略规则排除。历史快照只描述验收时文件；整理后的归档核对与资料处理见[资料索引](../RSKEY-FPM583F-REFERENCES.md#上传与本地保留)。必要原件的身份如下：

| 原件（相对 `raw/`） | SHA256 |
| --- | --- |
| `fingerprint-standard-client.log` | `46380b146013c78f4b69d4d1a284eab4e275f4e31fe6a705d94d9a40b9cf2d19` |
| `baseline-standard-client.log` | `5d25a3972d0058523c5365e495d919b60c530e6090c4ad59788a3cacbee9ebed` |
| `fingerprint-kani.log` | `6ccea7ab9d72bb764d9b1bbeb9e937b967e287ea44aa6326457b5b66e7a7d56f` |
| `gate-expiry-mutation.log` | `1588e3885773950b47eb4bd23c3eea054a9bedd44f0d2a9ff865ff9a43f8d8cc` |
| `gate-source-mutation.log` | `46a816c7c36aac0b50592c616fee8d1c5bcc46ce0721c10b12fcef9b19f844ca` |
| `gate-final.log` | `0e997489221399d0fe382183a605fcb5aea1ead5b4a56c5f90a7a54859eb6610` |

保存的 raw ELF 均未启用 display/no-touch、未应用分区表，只用于资源审阅。`check.sh` 最后留下 no-touch 测试镜像；设备候选须按目标配置重建并应用分区表。没有刷写、生产签名、提交或推送。
