#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Check the loadable build marker before a firmware ELF is packaged."""

import argparse
import pathlib
import struct
import subprocess
import sys

MAGIC = b"RSKBUILD"
VERSION = 1
RECORD = struct.Struct("<8sIIII")
FEATURES = ("no-touch", "fingerprint", "display")
PROFILES = {"default": 0, **{name: 1 << bit for bit, name in enumerate(FEATURES)}}
NO_TOUCH = PROFILES["no-touch"]


def read_marker(elf):
    done = subprocess.run(
        ["arm-none-eabi-objcopy", "-O", "binary", "--only-section=.rsk_build", str(elf), "/dev/stdout"],
        capture_output=True, check=True,
    )
    if len(done.stdout) != RECORD.size:
        raise ValueError("missing or malformed .rsk_build marker")
    magic, version, flags, flash_size, kvmain = RECORD.unpack(done.stdout)
    if magic != MAGIC or version != VERSION or flags & ~sum(PROFILES.values()):
        raise ValueError("unrecognized .rsk_build marker")
    if flags & PROFILES["fingerprint"] and flags & PROFILES["display"]:
        raise ValueError("incompatible fingerprint/display marker")
    if not flash_size or not 0 < kvmain < flash_size:
        raise ValueError("invalid flash geometry in .rsk_build marker")
    return flags, flash_size, kvmain


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-no-touch", action="store_true",
                        help="require a no-touch test image instead of a touch image")
    parser.add_argument("--expect", choices=PROFILES, help="require these build feature flags")
    parser.add_argument("elf", type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        flags, flash_size, kvmain = read_marker(args.elf)
        if bool(flags & NO_TOUCH) != args.test_no_touch:
            raise ValueError("no-touch test image refused" if flags & NO_TOUCH
                             else "expected a no-touch test image")
        if args.expect is not None and flags != PROFILES[args.expect]:
            raise ValueError(f"build feature flags {flags:#x} do not match {args.expect}")
    except (OSError, subprocess.CalledProcessError, ValueError) as error:
        reason = error.stderr.decode(errors="replace").strip() if isinstance(error, subprocess.CalledProcessError) else str(error)
        print(f"firmware-identity: {args.elf}: {reason}", file=sys.stderr)
        return 1
    enabled = ",".join(name for bit, name in enumerate(FEATURES) if flags & (1 << bit)) or "default"
    print(f"firmware-identity: {args.elf}: {enabled}; flash={flash_size}, KVMAIN={kvmain}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
