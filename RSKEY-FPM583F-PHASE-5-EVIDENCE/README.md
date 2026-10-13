<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# 第五阶段验收证据

2026-10-13。离线软件范围验收完成，原始完整 gate 退出 0；实现、策略及实物边界见[执行记录](../RSKEY-FPM583F-PHASE-5-RECORD.md)。本页保留验收必要摘录与原件身份，完整日志和 raw ELF 本地忽略。

## 标准客户端与主机

固定 python-fido2 2.2.1，2MB 几何，原 FIDO、目录、PIN/UV 及 RP 验签均实际执行；只有模块及传输是隔离模拟设施。

```text
39 passed in 401.00s
42 passed in 37.99s
24 passed in 7.87s
```

分别为[生物管理与字节协议](../tests/fpm_client/test_protocol.py)（21 个原管理用例加 18 个字节用例）、阶段三指纹、阶段二基线。主机指纹协议/执行路径 31 项、FIDO 指纹路径 18 项通过。

复查后将确认过的匹配 `Started` 保持至当前请求结束，防止先变为 `Unknown` 再错误退款。失败复现的实际结果为 `Unknown`、预期为 `Started`；修正后上述 31/18 项重跑通过，相关标准客户端负向路径复跑 `5 passed, 34 deselected in 86.74s`。

编解码的独立完整向量见[主机测试](../crates/rsk-device/src/fingerprint_protocol_tests.rs)，来自手册 p16/p22/p26。模型第六次返回 PROC=60，成功依据是用户确认的六有效样本及最终保存/目录提交，不伪造百分比完成。覆盖未知保存/删除、取消、三条授权路径、原公钥连续性、全库 Reset 与保留标记后的恢复。

## 证明与短 fuzz

Kani 0.67.0 的[命令编码不变量](../crates/rsk-device/src/fingerprint_protocol_kani.rs)：

```text
0 of 202 failed (4 unreachable)
VERIFICATION:- SUCCESSFUL
Complete - 1 successfully verified harnesses, 0 failures, 1 total.
```

[帧目标](../fuzz/fuzz_targets/fpm_frames.rs)在固定 nightly fuzz 环境、有手册帧种子的短运行：

```text
#20000 DONE cov: 79 ft: 270 corp: 50/2617b
Done 20000 runs
```

这不是完整 Kani tier 或长期 fuzz 活动，没有关闭指纹构建的既有形式 gap。

## 保护的反向验证

每行临时放宽一处保护，格式化后运行原始 `./scripts/check.sh`，均在实际 `test (host)` 行以 rc=101 停止；随后按 SHA256 核对逐字节恢复。正常基线和最终 gate 分别检查正向控制；没有以直接调用 helper 代替 gate。

| 保护 / 临时改动 | 实际失败断言及方向 | 正向控制 |
| --- | --- | --- |
| [应用层校验](../crates/rsk-device/src/fingerprint_protocol.rs)：`checksum(app) == 0` → `<= 1` | `both_checksums_and_all_header_lengths_are_checked_before_indexing`：损坏帧实际错误为 `None`，预期 `Some(ApplicationChecksum)`，即应拒绝却被接受；112 passed / 1 failed | 独立手册帧、合法帧切分和恢复后的合法帧 |
| 同文件命令回显：严格比较两字节 → 首字节相同即放行 | `short_success_unknown_code_wrong_command_and_slot_are_refused`：合法长度/校验的另一命令帧实际为 `Ok(Match { matched: true, slot: 3, … })`，预期 `Err(Command)`；112 passed / 1 failed | 同一独立手册 MATCH 帧的正确命令可解码 |
| [匹配阶段](../crates/rsk-fido/src/fingerprint.rs)：已为 `Started` 时只允许保持该状态 → 允许 `NotAttempted` | `no_attempt_evidence_cannot_override_a_started_match_or_an_old_operation`：实际 `NotAttempted`，预期 `Started`，即已开始事实被错误清除；732 passed / 1 failed / 1 ignored | 当前未知阶段可由明确无手指结果细化并退款；旧操作不能修改新操作 |

第一轮错误命令改动先触发了 `Err(Length)` 与 `Err(Command)` 的分类差异，不能证明错误响应被接受，未计入验收。调整用例顺序后，上表记录的是实际错误放行。所有临时改动已恢复；最终 codec 的 SHA256 为 `a9f1293c5b2e75b4d6669c729e8cd67f14ee336bfdf946556d025ad3e2e301e4`，FIDO 指纹模块为 `3db4b1cc885f9a955cf7047462aefb1943abe5b8c60913ad423297c930c44a57`。

## 完整 gate

正常源码恢复后从头执行原始 gate，退出码 0。必要摘录：

```text
3714 passed in 686.32s (0:11:26)
docs-constants: ok (97 values checked against 991 constants — 15 docs, 70 tests, 12 metadata)
312 passed in 0.69s
45 passed in 0.03s
no leaks found
ALL CHECKS PASSED
```

原 gate 的默认、显示与其他配置、镜像身份/分区/签名测试、形式引用、证据与脚本行均通过。签名仅为既有测试的临时随机材料，没有生产签名或设备动作；无屏 2MB 指纹资源另行实际构建，数值见执行记录。Kani/fuzz 新项的 roster 与生成页面同步，没有关闭 37 个既有指纹形式 gap，也没有降低 gate 门槛。

完成状态写回后，文档构建与现有离线链接检查为 730 项、0 错误；本轮工程/阶段文档链接为 87 项、0 错误。公开声明与运行计数检查再次通过。

## 复现

```sh
export NIX_CONFIG='experimental-features = nix-command flakes'
nix develop --no-update-lock-file -c bash scripts/bio-client.sh
nix develop --no-update-lock-file -c bash scripts/fingerprint-client.sh
nix develop --no-update-lock-file -c bash scripts/standard-client.sh
nix develop --no-update-lock-file -c cargo test -p rsk-device -p rsk-fido \
  --target x86_64-unknown-linux-gnu fingerprint
nix develop --no-update-lock-file -c cargo kani -p rsk-device --features kani-soft \
  --harness capture_encoding_preserves_length_and_both_checksums
nix develop .#fuzz --no-update-lock-file -c cargo fuzz run fpm_frames -- \
  -runs=20000 -max_len=2048
nix develop --no-update-lock-file -c env HOST_TARGET=x86_64-unknown-linux-gnu \
  CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true ./scripts/check.sh
```

Kani 需要固定 0.67.0 安装及配套工具路径；本轮使用已有本地安装的 Cargo 入口。新 clone 的 fuzz corpus 可由主机测试中的手册向量重建。

## 本地原件身份

以下日志名相对本阶段 `raw/`，ELF 名相对 `build/`；日志耗时、进程和路径在复跑时会改变。

| 原件 | SHA256 |
| --- | --- |
| `gate-final.log` | `4cc78d0fd200e0ffba430409736efc3bc3f677d903f874bfe66cd048c0fb96c1` |
| `bio-client-final.log` | `4a2920215d3f8941ceef01a096ed0fa9a7e0bc8ec9292bd05ca6b01f5d9fd11c` |
| `fingerprint-client.log` | `1b9a40987bba99d8591c49dc7731c4873bb45a9c3ced99d330841ad5268d3800` |
| `standard-client.log` | `baeb9eebc65d1c48f150425876f814f7fd420d50924f6040e76ab628708cbd88` |
| `host-protocol-final.log` | `f6329c5ec4b10e328fc8a0e2a1327bc0bcba639a2a3fdc3ef86316c251ccea81` |
| `wire-retry-final.log` | `0f32cbbb4ff9e9315426482eb7d7b2173264a49a494ee4ad6e68fecf6cf02018` |
| `kani.log` | `e639d0f1d4b23d56eb604301ba72f71cf1cb2d7802bc9be762b60a5152612dd8` |
| `fuzz-seeded.log` | `dbe981b01551ea159223d6eeb5c88312206c850ac08c77317eb494e3bb500905` |
| `mutant-checksum.log` | `0825ae4f72fe060f4cdef8a5d0e0505bf7f44876c27128cc3462219b2f48fada` |
| `mutant-command.log` | `8e6b2ef409d1ec4fa64208c63becf8d1d575f20f3722725f0aa84d57cd42df97` |
| `mutant-no-attempt.log` | `e8fe50899a6bf6f1c9fcd94a38467a4debb33a8b9ef7d118f3974787006e476d` |
| `resources-final.log` | `2121cfa1b4d12568e5e3f89ae2fdc776a91ab6e0d8e084db501d422465062426` |
| `docs-final.log` | `fe7b237ad26a05d9dd5d8f09dfe547612d35aa25814b5ebda7fcafe19598ed97` |
| `project-links-final.log` | `6f0201193313c281254797246cbbf497f294dbdf5172eaf11fa9f04d8b45e5cb` |
| `source-final.sha256` | `f01e7e9bfebf1a27b4feae71cf2ecc5edce7fed5e888ea09f84ce35efb53bc70` |
| `default.elf` | `498b13a09a4f14ff70bc05ac83431fcbc3ca68ed08e61aae99f09e69f72e9b21` |
| `fingerprint.elf` | `051eed13c4a225fd6aaa3ed57d7b2b82591a37bc24b2e17a28e096dc6b3c56a9` |

两份 raw ELF 都没有分区表，只供资源审阅；完整 gate 留下 no-touch 测试镜像。根 Cargo、flake 与模拟器锁定文件身份分别保持 `e0719d516ac008462724a853d57b5c0fb25425d1de72e2a19966c9a7f0912481`、`329b2d37efcae49a4dec8a14d28a8257304d999f78f6cfdd4e219b7d944b281b`、`7fb1e8eba983b8eaeb8a509238fb9498f081725b9a57e4bf9748e05230f7adf0`；没有新增依赖或升级固定项。

`source-final.sha256` 记录本轮 54 个改动文件的最终身份，含原有用户忽略规则；为避免循环哈希，排除本证据摘要。可从仓库根目录用 `sha256sum -c RSKEY-FPM583F-PHASE-5-EVIDENCE/raw/source-final.sha256` 核对本机快照。
