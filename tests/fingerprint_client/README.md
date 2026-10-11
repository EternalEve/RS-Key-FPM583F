<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# Fingerprint authorization integration tests

Run `nix develop -c ./scripts/fingerprint-client.sh`. It builds an isolated
`fingerprint-fixture` emulator and copies the binary before running the phase-two
pinned python-fido2 2.2.1 environment. It never enumerates or opens hardware.

The fixture replaces sensor observations and slot authorization only. Standard
SDKs encode CTAP and PIN/UV protocols; actual applet dispatch, storage, token
cryptography and credential signing run normally. RP verification checks the
stored credential's original public key. HID uses TCP reports; the CCID shim
encodes card frames, APDU chains and GET RESPONSE without implementing CTAP.

The emulator must bind loopback, use `--touch`, own its `--store`, and omit
display/USB-IP. `--fingerprint-init` provisions its test counter once and is
removed before restart. The event file is reread for each operation:
`kind input_ms processing_ms drain_ms`. Delays are bounded to 30 s. Kinds are
`match`, `unknown`, `nomatch`, `stale`, `generation`, `communication`,
`input-timeout`, `unknown-timeout` and `wait`. Only the fixture's current slot 7
and binding generation can authorize. Matching and idle events are observable
without logging PINs, tokens or template contents.

The matrix covers both entries, direct UV and UV/PIN tokens, failure without PIN
fallback, shared persistent retries, independent PIN soft lock, old credentials
across enabling fingerprint input, UP-only/silent requests, input/matching
CANCEL, draining and PIN fallback, channel ownership, HID INIT, partial reports,
U2F and both APDU chain directions. These are software tests; actual 583F packets,
IRQ/XIP interaction, TOUCHOUT freshness, USB enumeration and browser interop
remain hardware/protocol acceptance work.
