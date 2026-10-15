# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Deliberate faults for the owned E1 diagnostic wrapper; select one at a time."""

import os
import signal
import time
from pathlib import Path

TERM_DELIVERY_WINDOW = 0.1


def test_e1_injected_runner_failure():
    assert False, "E1 deliberate assertion: verify runner failure propagation"


def test_e1_injected_outer_term():
    target = int(os.environ["RSK_E1_SIGNAL_TARGET"])
    assert target > 1
    ancestor = os.getppid()
    while ancestor > 1 and ancestor != target:
        status = Path(f"/proc/{ancestor}/status").read_text()
        ancestor = int(next(line.split()[1] for line in status.splitlines() if line.startswith("PPid:")))
    assert ancestor == target, "signal target must own this diagnostic process"
    args = Path(f"/proc/{target}/cmdline").read_bytes().split(b"\0")
    wrapper = Path(os.fsdecode(args[1])).resolve(strict=True)
    evidence = Path(__file__).resolve().parents[1] / "target/fpm-review-evidence"
    assert wrapper.name == "run.sh" and wrapper.parent.parent == evidence
    print(f"injected outer SIGTERM: sender={os.getpid()} target={target}", flush=True)
    os.kill(target, signal.SIGTERM)
    time.sleep(TERM_DELIVERY_WINDOW)
