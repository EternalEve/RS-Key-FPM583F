# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Owned 2 MB emulator with the UART driver's byte-level module fixture."""

import importlib.util
import pathlib
import socket
import sys

import pytest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "fingerprint_client"))
_spec = importlib.util.spec_from_file_location("fpm_transport_fixtures", pathlib.Path(__file__).resolve().parents[1] / "fingerprint_client/conftest.py")
_fixtures = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_fixtures)
binary = _fixtures.binary


@pytest.fixture(params=[False, True], ids=["constant", "demand"])
def wire_emulator(binary, tmp_path, request):
    script = tmp_path / "module.events"
    script.write_text("good\n")
    with socket.socket() as reservation:
        reservation.bind(("127.0.0.1", 0))
        port = reservation.getsockname()[1]
    emu = _fixtures.FingerprintEmulator(binary, tmp_path / "wire.store", [
        "--ccid-port", str(port), "--fingerprint-script", str(script),
        "--fingerprint-management", "--fingerprint-protocol",
    ] + (["--fingerprint-power"] if request.param else []))
    emu.script, emu.ccid_port = script, port
    emu.controlled = request.param
    emu.inventory = script.with_suffix(".inventory")
    try:
        emu.start()
        yield emu
    finally:
        emu.stop()
