#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors

"""Exercise real presence without creating credentials or changing a PIN.

    nix develop -c python tests/50_user_presence_button.py --popup
    nix develop -c python tests/50_user_presence_button.py --mode cancel
    nix develop -c python tests/50_user_presence_button.py --mode timeout

Requires a touch-enabled image. Press only BOOTSEL in press mode; leave BOOTSEL
and TOUCHOUT untouched for cancel/timeout. A bounded host deadline also catches
a worker stuck opening the fingerprint touch window despite ongoing keepalives.
The optional desktop popup requires zenity.
"""
import argparse
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from ctaphid import (  # noqa: E402
    CTAPHID_CBOR,
    CTAPHID_INIT,
    CTAPHID_KEEPALIVE,
    REPORT_LEN,
    find,
    hid,
    write,
)

CTAPHID_CANCEL = 0x91
AUTHENTICATOR_SELECTION = 0x0B
UPNEEDED = 0x02
CTAP_OK = 0x00
KEEPALIVE_CANCEL = 0x2D
USER_ACTION_TIMEOUT = 0x2F
PRESENCE_TIMEOUT_S = 30
RESPONSE_GRACE_S = 5
CANCEL_AFTER_S = 1


def read_until(dev, deadline):
    while time.monotonic() < deadline:
        report = bytes(dev.read(REPORT_LEN, 100))
        if report:
            assert len(report) == REPORT_LEN, "short HID report"
            return report
    raise TimeoutError("no final presence response within the host deadline")


def init(dev):
    nonce = os.urandom(8)
    broadcast = b"\xff" * 4
    write(dev, broadcast + bytes([CTAPHID_INIT, 0, len(nonce)]) + nonce)
    report = read_until(dev, time.monotonic() + RESPONSE_GRACE_S)
    assert report[:7] == broadcast + bytes([CTAPHID_INIT, 0, 17]), "INIT failed"
    assert report[7:15] == nonce, "INIT nonce mismatch"
    return report[15:19]


def selection(dev, cid, mode, popup):
    write(dev, cid + bytes([CTAPHID_CBOR, 0, 1, AUTHENTICATOR_SELECTION]))
    start = time.monotonic()
    deadline = start + PRESENCE_TIMEOUT_S + RESPONSE_GRACE_S
    up_since = None
    cancelled = False
    prompt = None
    try:
        while time.monotonic() < deadline:
            now = time.monotonic()
            if mode == "cancel" and up_since is not None and not cancelled:
                if now - up_since >= CANCEL_AFTER_S:
                    write(dev, cid + bytes([CTAPHID_CANCEL, 0, 0]))
                    cancelled = True
                    deadline = now + RESPONSE_GRACE_S
                    print("   CANCEL sent", flush=True)
            report = read_until(dev, deadline)
            assert report[:4] == cid, "reply on another channel"
            command = report[4]
            if command == CTAPHID_KEEPALIVE:
                assert report[5:7] == b"\x00\x01", "malformed keepalive"
                if report[7] == UPNEEDED and up_since is None:
                    up_since = time.monotonic()
                    print("   UPNEEDED: press BOOTSEL now" if mode == "press"
                          else "   UPNEEDED: leave both inputs untouched", flush=True)
                    if popup:
                        prompt = subprocess.Popen([
                            "zenity", "--info", "--title=现在按下 BOOTSEL",
                            "--text=现在请只按实体 BOOTSEL，保持约 1 秒后松开。\n不要碰指纹模块。",
                            f"--timeout={PRESENCE_TIMEOUT_S}",
                        ])
                continue
            assert command == CTAPHID_CBOR, f"unexpected HID command {command:#x}"
            assert report[5:7] == b"\x00\x01", "selection must return one status byte"
            assert up_since is not None, "presence completed without UPNEEDED"
            elapsed = time.monotonic() - start
            expected = {"press": CTAP_OK, "cancel": KEEPALIVE_CANCEL,
                        "timeout": USER_ACTION_TIMEOUT}[mode]
            assert report[7] == expected, f"CTAP status {report[7]:#x}, expected {expected:#x}"
            if mode == "cancel":
                assert cancelled, "request ended before CANCEL was sent"
            if mode == "timeout":
                assert elapsed >= PRESENCE_TIMEOUT_S - 1, "presence timed out early"
            print(f"PASS: {mode}, CTAP {expected:#04x} after {elapsed:.2f}s")
            return
        raise TimeoutError("presence kept sending keepalives past the host deadline")
    finally:
        if prompt is not None:
            if prompt.poll() is None:
                prompt.terminate()
            prompt.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("press", "cancel", "timeout"), default="press")
    parser.add_argument("--popup", action="store_true", help="show BOOTSEL timing on the desktop")
    args = parser.parse_args()
    if args.popup and args.mode != "press":
        parser.error("--popup requires --mode press")
    if args.popup:
        ready = subprocess.run([
            "zenity", "--question", "--title=BOOTSEL 测试准备",
            "--text=点击开始后，等待按键提示。只按实体 BOOTSEL，不碰指纹模块。",
            "--ok-label=开始测试", "--cancel-label=取消",
        ])
        if ready.returncode:
            raise SystemExit("test cancelled before opening the device")
    info = find()
    if not info:
        raise SystemExit("No selected FIDO HID device found")
    dev = hid.device()
    dev.open_path(info["path"])
    try:
        selection(dev, init(dev), args.mode, args.popup)
    finally:
        dev.close()


if __name__ == "__main__":
    main()
