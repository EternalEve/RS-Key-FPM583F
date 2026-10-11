# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Fresh synthetic storage for each integration case, always owned by the test."""

import hashlib
import importlib.metadata
import json
import os
import pathlib
import platform

import pytest

from session import Emulator

ROOT = pathlib.Path(__file__).resolve().parents[2]


@pytest.fixture(scope="session")
def binary():
    assert importlib.metadata.version("fido2") == "2.2.1", "use the pinned client environment"
    path = pathlib.Path(os.environ.get(
        "RSK_CLIENT_EMU", ROOT / "tools/emu/target/x86_64-unknown-linux-gnu/debug/rsk-emu"
    )).resolve(strict=True)
    print("environment " + json.dumps({
        "python": platform.python_version(),
        "packages": {name: importlib.metadata.version(name)
                     for name in ("fido2", "cryptography", "pytest", "cffi", "pycparser")},
        "emulator": str(path),
        "emulator_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    }))
    return path


@pytest.fixture
def emulator(binary, tmp_path):
    emu = Emulator(binary, tmp_path / "session.store")
    try:
        emu.start()
        yield emu
    finally:
        emu.stop()
