<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# Standard biometric client acceptance

Run `nix develop -c ./scripts/bio-client.sh`. This reuses phase two's pinned
python-fido2 2.2.1 environment and phase three's deadline-bounded HID/CCID
transport. It requires the emulator's `fingerprint-fixture` feature, loopback,
`--touch`, an owned synthetic store and `--fingerprint-management`; production
binaries have no scripted module. It never enumerates hardware. The
`bio-fixture-2mb` profile uses 896 KiB main and 128 KiB counter partitions; the image including its header is 1,048,592
bytes. The earlier fixture and ordinary emulator keep their original geometry.

The production FIDO directory, PIN/BE authorization, standard 0x09 command,
credential derivation and RP signature validation remain real. A separate
`.inventory` file stands for module template storage; only final scripted saves
add slots. Its three-sample capability and feedback are test facts. Script
lines use phase three's event format; management adds `poor-quality`,
`no-transition`, `no-activity`, `save-failure`, `save-unknown`, `delete-failure`
and `clear-failure`. `save-unknown` persists the slot but loses the result.

Restart reopens both stores. Tests cover protocols 1/2, both transports,
management exceptions and permissions, UTF-8 byte limits, cancellation, saved
residue, disabled deletion, slot reuse, last-template PIN fallback, full lists
and Reset recovery. Host tests alongside `bio_enrollment.rs` additionally
inject failures through the actual Fs backend at its durable boundaries.
Normal file reopening is software persistence evidence; hardware response
trust, flash power cuts and UART timing remain subsequent acceptance work.
