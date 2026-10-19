<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->

# FPM583F byte-level acceptance

Run inside the fixed development shell:

```sh
bash scripts/bio-client.sh
```

This runner executes both the phase-four scripted management suite and these
tests. The latter select `--fingerprint-protocol` with `bio-fixture-2mb`, an
owned store, loopback endpoints and terminal presence. They use the pinned
python-fido2 2.2.1 API and the real FIDO business, directory and cryptography.
The module fixture independently decodes requests and builds UART replies;
firmware and the fixture use the same production driver and response decoder.

Each UART case runs against constant supply and `--fingerprint-power`. The latter
starts off, retains nonvolatile inventory across cycles and destroys temporary
samples at power-off. Transition logs come from the module stream, below FIDO.
Power regressions additionally cover idle/PIN/UP-only processing, session release,
sleep response loss and startup failure. They do not emulate physical TOUCHOUT;
the host-tested event window covers old/duplicate pulses and high-at-arm rejection.

Six successful manual captures initiate a single save. The fixture's sixth
capture reports `PROC=60`, deliberately separating the approved sample-count
policy from percentage. Failed samples retain the remaining count. Quality,
no-finger and excessive/insufficient movement feedback preserve prior good
samples through a retry, final save and directory authorization. Save/delete
response loss preserves the production pending/deleting recovery boundaries;
the fixture never writes main-controller authorization records.

Verification also covers a module `NO_FINGER` result followed by later contact
in the same CTAP request: the script changes from `nofinger` to `present` only
after the first match-result query. The response must carry UV and a valid
signature, and successful verification must leave eight UV retries.

Logs contain module command IDs, test results and owned paths. Synthetic store
and inventory files are confined to the runner's temporary session. These are
software vectors, not hardware captures or evidence of physical USB, touch,
UART ordering or nonvolatile durability.
