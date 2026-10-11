<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# 第二阶段验收证据

2026-10-11。阶段结果与范围见[执行记录](../RSKEY-FPM583F-PHASE-2-RECORD.md)。完整日志留在本地 `raw/`；以下仅保留缺陷方向、最终结果及原件身份，摘录逐行取自实际输出。

## Restart

正常重启后旧凭据未被恢复，真实 getAssertion 返回 NO_CREDENTIALS。补充启动 `Fs::scan()` 后，原重启用例与完整客户端矩阵通过；断言见[认证测试](../tests/standard_client/test_ceremony.py)。修复前：

```text
E   fido2.ctap.CtapError: CTAP error: 0x2E - NO_CREDENTIALS
1 failed in 1.21s
```

## Exit

runner 原先接受提前 SIGKILL、提前 SIGTERM 和未声明的退出 0。三项回归均因“应拒绝但没有抛错”失败；修复后仅接受明确预期的停止，资源先关闭。对应[退出测试](../tests/standard_client/test_session.py)。修复前与修复后依次为：

```text
_________________ test_unexpected_emulator_exit_is_refused[9] __________________
E   Failed: DID NOT RAISE <class 'AssertionError'>
3 failed, 21 passed in 7.95s
24 passed in 7.98s
```

## Gate results

初次完整入口分别暴露 runner 未登记、引用坐标漂移；两项已按原要求修正，没有更改模型语义或弱化 gate。退出检查修复后的最终验收运行原始 `scripts/check.sh`，退出 0，gitleaks 无泄漏。

### Final gate

```text
3713 passed in 664.76s (0:11:04)
312 passed in 0.66s
45 passed in 0.04s
ALL CHECKS PASSED
```

## Reproduction

从仓库根目录运行；使用固定 Nix 环境及测试专用 python-fido2 2.2.1：

```sh
export NIX_CONFIG='experimental-features = nix-command flakes'
nix develop --no-update-lock-file -c ./scripts/standard-client.sh
nix develop --no-update-lock-file -c env HOST_TARGET=x86_64-unknown-linux-gnu \
  CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true ./scripts/check.sh
```

夹具限 loopback、独立合成 store 和 `--touch`，不枚举实体设备。普通文件重启持久性不等于断电耐久；实物 USB、运行时栈、OTP 与启动恢复仍待对应阶段验证。

## Local archive

根目录原有七份小型日志与 `raw/` 副本逐字节一致，已去除重复公开副本；完整原件仍保留在被 Git 忽略的 `raw/`。下表只标识必要原件，不罗列重复构建和整理过程输出；新运行的随机数据、路径与耗时会改变日志哈希。

| 原件（相对 `raw/`） | SHA256 |
| --- | --- |
| `restart-before-fix.log` | `c7e39b0b548a586f04587f3a7dba91ebc0c7dac22905c080bdd3a500ef415a6e` |
| `exit-before-fix.log` | `bc0b3f53ded7753f4c5f4d4fe8f6f069678aa6d35306c1898dac4e5d9c27cd23` |
| `exit-after-fix.log` | `5053f9ea61028508af5e13462cd232e42ebc670d4f5fecd4698dfe4230869faf` |
| `exit-fix-gate-full.log` | `56fb6a00c89d5578dd5dbb44551e74eb5af90830bd4800413d2357e01eccce1c` |
