<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# 第二阶段验收证据

2026-10-11。保留缺陷复现、最终客户端测试与必要检查的小型原始日志。完整 gate 的重复编译输出仅作本地归档；以下摘录逐行取自原件，省略部分不代表额外测试结论。

## Gate results

四次完整入口运行的观察结果依次为：runner 登记遗漏（退出 1）、引用漂移（退出 1）、初次验收通过（退出 0）、退出检查修复后验收通过（退出 0）。最终运行使用仓库未修改的 `scripts/check.sh`，命令见[执行记录](../RSKEY-FPM583F-PHASE-2-RECORD.md)。

### Runner registration

原件 `gate-before-runner-registration.log`，第 10810–10812 行：

```text
== reproduction runner ==
FAIL runners: 'scripts/standard-client.sh' is claimed by nothing here — give it a phase or a reason
reproduce: 1 self-test failure(s)
```

### Citation drift

原件 `gate-before-citation-sync.log`，第 10842–10853 行：

```text
== formal citations ==
citation-gate:
  formal/RSKeySecurityState.tla: `tools/emu/src/device.rs:507` -> tools/emu/src/device.rs was locked to `let seeded = rsk_fido::seed::ensure_seed(&dev(),`, which is now at :508; the citation has drifted
  formal/README.md: `tools/emu/src/device.rs:507` -> tools/emu/src/device.rs was locked to `let seeded = rsk_fido::seed::ensure_seed(&dev(),`, which is now at :508; the citation has drifted
  scripts/security_trace.py: `tools/emu/src/device.rs:754-755` -> tools/emu/src/device.rs was locked to `crate::security_trace::POWER_CYCLE,`, which is now at :755; the citation has drifted
  scripts/security_trace.py: `tools/emu/src/device.rs:754-755` -> tools/emu/src/device.rs was locked to `crate::security_trace::POWER_CYCLE,`, which is now at :755; the citation has drifted
  scripts/test_security_trace.py: `tools/emu/src/device.rs:754-755` -> tools/emu/src/device.rs was locked to `crate::security_trace::POWER_CYCLE,`, which is now at :755; the citation has drifted

The model's `file.rs:line` citations are the only bridge between it
and the code it abstracts. One that no longer resolves sends the next
reader somewhere the claim was never true. Its phase-1 property tags
must also resolve model→code and code→model. Repair or drop the claim.
```

### Final gate

原件 `exit-fix-gate-full.log`，第 11142–11164 行：

```text
3713 passed in 664.76s (0:11:04)

== docs constants match code ==
docs-constants: ok (96 values checked against 957 constants — 14 docs, 70 tests, 12 metadata)

== pytest (tools/rsk) ==
........................................................................ [ 23%]
........................................................................ [ 46%]
........................................................................ [ 69%]
........................................................................ [ 92%]
........................                                                 [100%]
312 passed in 0.66s

== pytest (tests/interop) ==
.............................................                            [100%]
45 passed in 0.04s

== gitleaks (tree) ==
8:42PM INF 1602 commits scanned.
8:42PM INF scanned ~28467599 bytes (28.47 MB) in 2.39s
8:42PM INF no leaks found

ALL CHECKS PASSED
```

## Local archive

完整原件保存在本地 `raw/`，该目录由根 `.gitignore` 精确忽略，普通 Git 提交不包含它。克隆仓库后可运行执行记录中的命令生成新的日志；新的 PID、随机数据和耗时会改变校验值。下表用于核对本次原件，不把摘要当作完整运行日志。

| 原件（相对 `raw/`） | 行数 | 字节数 | SHA256 |
| --- | ---: | ---: | --- |
| `citation-sync-check.log` | 34 | 3177 | `8f30f5da4c4fb37ae7f1e46ec6cb81a1c85dc632ad107cb826c50d6450beb73b` |
| `docs-check.log` | 18 | 848 | `a43b8d32f73c27e94eb08968c5d5fa0f91b47ba318f55558520870a56917b6c3` |
| `emu-host-tests.log` | 126 | 8593 | `d3c9a42d3690025cd8e6e551fc38fd586ac9e634a54b1ae7a0cd4aeefff38d7e` |
| `exit-after-fix.log` | 46 | 7240 | `5053f9ea61028508af5e13462cd232e42ebc670d4f5fecd4698dfe4230869faf` |
| `exit-before-fix.log` | 66 | 8859 | `bc0b3f53ded7753f4c5f4d4fe8f6f069678aa6d35306c1898dac4e5d9c27cd23` |
| `exit-fix-docs-check.log` | 38 | 2351 | `12580dfa1e41c7c69e43f655ce71a92f4fe0882770665634c79d4bc475fd983c` |
| `exit-fix-gate-full.log` | 11164 | 704069 | `56fb6a00c89d5578dd5dbb44551e74eb5af90830bd4800413d2357e01eccce1c` |
| `exit-fix-record-check.log` | 21 | 1598 | `d97261ea94ac5caea89d3e8809a598448b6bd7a7370653e3ebc2da0254877e77` |
| `final-docs-check.log` | 38 | 2351 | `d56058e6bd5381c08d1b7f0f93f543ae0fb29f80bf82cce01e8e8082d5ae9dc6` |
| `gate-before-citation-sync.log` | 10853 | 670132 | `3e27d733029fd52d3081a498968698c55f97f6e620b79bef6d45821f2dd9bf12` |
| `gate-before-runner-registration.log` | 10812 | 667806 | `6102e93bcf60f1da435e989041aa73a363ac6f44b876d0ff9ce00a4ba44c2c15` |
| `gate-full.log` | 11147 | 703019 | `3d6687b03eb476b09f0c4f7e6b4182bbab0edd25d071c977833daa91f5090143` |
| `restart-before-fix.log` | 37 | 3215 | `c7e39b0b548a586f04587f3a7dba91ebc0c7dac22905c080bdd3a500ef415a6e` |
| `runner-registration-check.log` | 36 | 1912 | `cc176b6c1bb5e770fc7327297d74090a215c0618db0a67d2cbbe6c58d28d51f5` |
| `standard-client.log` | 28 | 5642 | `a32dfab72a9b667f9ffc6db464e670b69024cc66da4d277d9b3ffa7fd949caf6` |
| `organization-docs-check.log` | 40 | 2431 | `c833fa54d93c971234ed19b42fd4c960339c1d15fb1a6bbd07875f25c5768e03` |
| `organization-gate-full.log` | 11163 | 703995 | `fdf3bd7039550a3ba09d78437405d62698d23afd54a7730048d67c501913e77e` |

## 整理复核

2026-10-11。整理后再次从头运行原始完整 gate，退出 0，`ALL CHECKS PASSED`，gitleaks 无泄漏。三组 Python 测试的原始结果为：

```text
3713 passed in 671.61s (0:11:11)
312 passed in 0.66s
45 passed in 0.03s
```

文档构建、729 个站点链接及 74 个阶段记录/证据链接检查均退出 0、0 Errors；citation、claims、run-count、SPDX、bcd 守卫通过。15 份原件校验值、7 份保留日志的字节一致性及 3 处摘录的原始行范围已核对。新增完整检查日志同样保存在本地 `raw/`；校验值见上表。
