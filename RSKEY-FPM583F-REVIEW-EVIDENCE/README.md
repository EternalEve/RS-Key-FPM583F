<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# 2026-10-15 审查复现

[审查记录](../RSKEY-FPM583F-REVIEW-2026-10-15.md)中 R1–R3、R5 的离线证据见 [review.rs](review.rs)，四项均已随修复改为验证正确行为。正式 R1 回归位于 [fingerprint_presence_tests.rs](../crates/rsk-device/src/fingerprint_presence_tests.rs) 和 [presence_tests.rs](../crates/rsk-device/src/presence_tests.rs)，R2 回归位于 [clientpin_fingerprint_tests.rs](../crates/rsk-fido/src/clientpin_fingerprint_tests.rs)，R3 回归位于 [fingerprint_driver_tests.rs](../crates/rsk-device/src/fingerprint_driver_tests.rs)，R5 回归位于 [bio_isolation_tests.rs](../crates/rsk-fido/src/bio_isolation_tests.rs)、[bio_directory_tests.rs](../crates/rsk-fido/src/bio_directory_tests.rs)、[ccid_bio_tests.rs](../crates/rsk-device/src/ccid_bio_tests.rs) 和 [pin_tests.rs](../crates/rsk-display/src/pin_tests.rs)，均由原始主机门禁执行。R3 另有 [标准客户端回归](../tests/fpm_client/test_protocol.py)，覆盖常供电与按需供电的真实 BioEnrollment 会话。初轮缺陷日志继续保存在本地，其通过曾表示现象复现，不能当作正确行为的回归。

复现直接使用生产 crate 和现有字节模块夹具，没有复制或修改被审查的实现。R1 的时钟后端使用生产 Arbiter，按固件 Board 的 processing/waiting/end 实现连接驱动；它不运行真实 IRQ。R2 使用真实 ClientPIN/ECDH/加密和 RAM Storage；R3 使用合法模块反馈及同样本重试；R5 从实际库存协调生成隔离目录，再注入空记录，通过无指纹上下文的真实 CTAP Reset 验证保护遗漏。所有 PIN、模块库存和时钟均为合成数据；没有设备入口。

在仓库根目录准备临时独立 Cargo manifest，沿用根锁文件中已有依赖版本；不向生产工程新增依赖：

```sh
review_work=$(mktemp -d /tmp/rsk-fpm-review.XXXXXX)
python3 - "$PWD" "$review_work" <<'PY'
import json
import shutil
import sys
from pathlib import Path

repo, work = map(Path, sys.argv[1:])
lines = [
    '# SPDX-License-Identifier: AGPL-3.0-only',
    '# Copyright (C) 2026 RS-Key contributors',
    '[package]', 'name = "rsk-fpm-review"', 'version = "0.0.0"',
    'edition = "2024"', '[dependencies]',
]
for name in ('rsk-device', 'rsk-fido', 'rsk-fs', 'rsk-crypto'):
    features = ', features = ["test-util"]' if name == 'rsk-fs' else ''
    lines.append(f'{name} = {{ path = {json.dumps(str(repo / "crates" / name), ensure_ascii=False)}{features} }}')
lines += [
    'minicbor = "=2.2.2"', '[workspace]', '[[test]]', 'name = "review"',
    'path = ' + json.dumps(str(repo / 'RSKEY-FPM583F-REVIEW-EVIDENCE/review.rs'), ensure_ascii=False),
]
(work / 'Cargo.toml').write_text('\n'.join(lines) + '\n')
shutil.copyfile(repo / 'Cargo.lock', work / 'Cargo.lock')
PY
nix develop --offline --no-update-lock-file -c env RSK_REPO_ROOT="$PWD" \
  CARGO_BUILD_JOBS=2 cargo test --offline --manifest-path "$review_work/Cargo.toml" \
  --target x86_64-unknown-linux-gnu -- --nocapture
```

必要观察：修复后的 R1 在确认排空且 ready、结束旧 dispatch 并开始新 dispatch 后为 `Ok(())`，新发出的取消仍返回 `Err(KeepAliveCancel)`；R2 连续九次正确 PIN 返回 `0x00`，PIN 次数保持 8，损坏的 UV 记录保持原样；R3 `0x0e/0x11/0x12` 均返回 `PoorQuality`、保留 1 个有效样本及剩余 5 次，继续采满后只保存一次；R5 有效隔离目录、非空坏记录及空坏记录均阻止无指纹 Reset 和种子使用，空坏记录返回 `Other` 并保持原样。四组均有合法正向控制。

完整编译、客户端、复现和资源日志保存在本地忽略的 `target/fpm-review-evidence/`，不包含生产密钥或设备采集。命令重跑时日志耗时和临时路径会变化。

E1 的信号跟踪、失败传播控制及再次采集方法见 [E1 排查记录](e1.md)。2026-10-16 按用户要求暂缓调查，保留未闭合状态及现有证据；历史两次退出 143 的发送方仍未确认，不能用本轮未复现替代归因。
