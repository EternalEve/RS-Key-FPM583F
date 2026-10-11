<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# FPM583F 资料与本地归档

计划引用的资料身份如下。模块 PDF 为用户提供的本地资料，暂不随源码上传；实现与实物核对仍须取得对应原件，本文不代替完整手册。

## FPM583F

- 原件：`FPM583F.pdf`，规格书 V1.0.2，封面日期 2021.12.23。
- 用途：电气、六针排线、标称容量及上电约束。
- SHA256：`27b19563afffec537b37cee7445c93b85af8da86900d666f5103005ead6c40d4`。

## FPM383C583F

- 原件：`FPM383C583F通用用户通信协议手册V1.2.pdf`，46 页；封面 V1.2.0 / 2020.09.11，型号写为 HLK-FPM383C，部分页眉为 V1.1.10。
- 用途：用户指定的 583F/383C 命令与报文依据；不从封面型号推断实物固件一致性。
- SHA256：`b3a42649f5c7e608d4c276a929000b1cfd5cce986cbc6a9a46a9d689175e96d9`。

报文能力、矛盾与待核对项见[第一阶段记录第 7 节](RSKEY-FPM583F-PHASE-1-RECORD.md#7-模块必要能力及候选资料17)。

## RP2350-Zero-CM

- 来源：[Waveshare 官方固定提交](https://github.com/waveshareteam/RP2350-Zero-CM/tree/fd66fd7e9106a4de42d5f1a4dc42938b4aa9ac59)。
- 本地原理图：`RP2350-Zero-CM.pdf`。
- SHA256：`8fc9ee2bf8c8aaabd3f0c9934e5ea009cdffe666ec9a90e1f54aa57093363686`。

## RP2350

- 来源：[Raspberry Pi 官方数据手册](https://datasheets.raspberrypi.com/rp2350/rp2350-datasheet.pdf)。
- 本次核对副本 SHA256：`a638d6780da919162660691bfa02c6dcf7c2a0b6e9f18e2d7519c80b664a2e39`。
- GPIO/E9 的适用条件见[第一阶段记录第 3 节](RSKEY-FPM583F-PHASE-1-RECORD.md#3-板级配置与预留13)；官方链接可能更新，核对历史结论须使用上述身份。

## 上传与本地保留

| 内容 | 处理 |
| --- | --- |
| 源码、固定依赖、测试、计划、验收摘要和必要缺陷摘录 | 随源码保留 |
| 两份模块 PDF、第一阶段完整证据目录 | 按既有精确规则留在本地 |
| 第二、三阶段 `EVIDENCE/raw/` | 保存完整日志；第三阶段同时保存整理前文档与历史校验清单，Git 忽略 |
| 第三阶段 `EVIDENCE/build/`、ELF/UF2、target、Nix result、book/site | 本地构建产物，Git 忽略 |
| 合成 `.store`、模拟器 SRAM、私钥与 secrets | 既有忽略规则覆盖；不作为源码交付物 |

公开证据见[第二阶段摘要](RSKEY-FPM583F-PHASE-2-EVIDENCE/README.md)和[第三阶段摘要](RSKEY-FPM583F-PHASE-3-EVIDENCE/README.md)。本地归档路径只作文字说明；克隆仓库后可按摘要中的命令重新生成日志。归档中的 SHA256 对应当次原件，耗时、PID 和路径变化会使新日志的哈希不同。

整理前的源文件与证据快照保留为历史记录；整理后的本地归档可从仓库根目录用 `sha256sum -c RSKEY-FPM583F-PHASE-3-EVIDENCE/raw/archive-snapshot.txt` 核对。该清单只用于本机归档，不声明当前文档仍与验收时同字节。
