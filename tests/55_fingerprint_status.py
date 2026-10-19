#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors

"""Read the non-secret fingerprint snapshot over the actual CTAPHID_MSG path."""

from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from rsk.fingerprint import INS_STATUS, parse_status, status_fields
from _device import find_fido2
from fido2.hid import CTAPHID


def main():
    device = find_fido2()
    if device is None:
        raise SystemExit("selected FIDO test device is absent")
    try:
        selected = device.call(CTAPHID.MSG, bytes.fromhex("00a4040005f000000001"))
        assert selected[-2:] == b"\x90\x00", "vendor SELECT failed"
        response = device.call(CTAPHID.MSG, bytes([0, INS_STATUS, 0, 0, 0]))
        assert response[-2:] == b"\x90\x00", f"snapshot SW={response[-2:].hex()}"
        print(status_fields(parse_status(response[:-2])))
        print("PASS: diagnostic snapshot; no module operation requested")
    finally:
        device.close()


if __name__ == "__main__":
    main()
