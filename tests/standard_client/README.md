<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# Standard client against the emulator

```sh
nix develop -c bash scripts/standard-client.sh
```

The runner builds the ordinary emulator with its frozen lockfile, then runs
python-fido2 **2.2.1** in a separate Nix Python environment. `env.nix` uses the
repository's locked nixpkgs revision and a hash-checked official PyPI wheel.
The library is a test-only dependency: it provides CTAPHID, CTAP2, ClientPin,
WebAuthn client validation and RP signature verification. No dependency is
added to the firmware or to the installed `rsk` CLI. Pytest arguments may be
appended, for example `-k restart`.

Each integration test starts its own process on an available **127.0.0.1** port,
with `--touch`, `--ccid-port 0` and a fresh absolute `--store` path. There is no
USB enumeration or fallback to hardware. Stdin stays open. A test replies only
after the current emulator approval prompt appears; replies are automated test
inputs, not evidence of human or physical presence. Cancellation uses the
standard library's event after its `UPNEEDED` callback, with no stdin approval.
Quick approvals may finish before a keepalive; waiting tests observe it explicitly.

`transport.py` implements only the library's `CtapHidConnection`. TCP carries
exactly 64-byte reports in each direction, without a hidapi report-ID prefix.
Reads handle fragmentation and coalescing; invalid output lengths are refused.
Every operation has a total deadline (10 seconds by default), including INIT,
GetInfo and PIN exchange. EOF, socket failure and deadline expiry close the
connection and keep their transport error. Requests are not replayed. Calls on
one connection are serial; a library cancellation consumes the original CTAP
terminal response before the next operation. A transport failure terminates
that test session instead of attempting to repair an unknown write result.

`ceremony.py` keeps two reusable boundaries: `Interaction` supplies synthetic
PINs and handles observed presence/cancel callbacks; `RelyingParty` uses
`DefaultClientDataCollector` and `Fido2Server` with their default origin checks.
Registration saves only public credential data and synthetic user association.
Discoverable authentication verifies the registered public key and user handle,
with required PIN UV. A separate non-discoverable allowList path proves UP-only
authentication without changing `alwaysUv` or `credProtect`. The private client
backend assertion detects silent CTAP1 fallback in the fixed library version.

The restart test stops the software process with SIGTERM after a completed
operation, remounts the same image, rebuilds all protocol objects and obtains a
fresh PIN token. It authenticates the old credential against its original public
key with a new RP challenge. It never registers a replacement. Storage write
errors fail the fixture; incompatible flash images are refused and preserved.

Cleanup accepts normal shutdown only when it sends SIGTERM to a running child
and observes that signal's exit status. A child that already exited fails,
including exit zero or an earlier SIGTERM. The incompatible-image test alone
declares its expected exit zero after checking the exact mount refusal and
preserved bytes. Connections, reader threads and pipes are closed before an
unexpected exit is reported; repeated cleanup is safe.

Negative tests require the expected failure cause: denial, cancellation,
missing/wrong PIN, another RP's credential, invalid origin/RP pairing, wrong
challenge, altered signature and missing required UV. Stream tests additionally
cover partial EOF, partial-report timeout, deadlines despite progress and closing
a waiting reader. No necessary test is silently skipped.

Logs contain versions, binary SHA256, process/port/store identity, capabilities,
public flags/counters and error names. They never print PINs, tokens, seeds or
private keys. The runner removes only its own temporary directory, including
the synthetic flash images. Failed cases still return a nonzero exit status.

The emulator's `--touch` backend claims `shows_confirm=true`, unlike a screenless
board, so PIN token permission consent also asks for terminal approval. It has
no built-in fingerprint UV. Its default flash geometry is 352/32 sectors, not
the 2MB board's 224/32. This suite covers shared business logic and ordinary file
persistence; physical USB, hardware timing, power-cut recovery, flash capacity,
fingerprints, browsers, OTP and secure boot require separate validation.

This is a separate emulator integration entry, like `emu-suites.sh`. Run the
unchanged `nix develop -c ./scripts/check.sh` gate as well; it uses the original
dev-shell Python and does not substitute for this standard-client suite.
The `emu` phase of `scripts/reproduce.sh` runs both emulator integration entries.
