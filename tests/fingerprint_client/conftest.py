# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Reuse the pinned client and owned process from phase two."""

import pathlib
import importlib.util
import socket
import sys

import pytest
from fido2.ctap import CtapDevice
from fido2.ctap2 import Ctap2

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "standard_client"))
from session import Emulator  # noqa: E402

_spec = importlib.util.spec_from_file_location("baseline_client_fixtures", pathlib.Path(__file__).resolve().parents[1] / "standard_client/conftest.py")
_baseline = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_baseline)
binary = _baseline.binary


class DeadlinedDevice(CtapDevice):
    def __init__(self, device, connection):
        self.device, self.connection = device, connection

    @property
    def capabilities(self):
        return self.device.capabilities

    @classmethod
    def list_devices(cls):
        raise AssertionError("owned test transport never enumerates hardware")

    def close(self):
        self.device.close()

    def call(self, cmd, data=b"", event=None, on_keepalive=None):
        if self.connection.deadline is not None:
            return self.device.call(cmd, data, event, on_keepalive)
        with self.connection.operation():
            return self.device.call(cmd, data, event, on_keepalive)


class FingerprintEmulator(Emulator):
    def start(self):
        super().start()
        self.device = DeadlinedDevice(self.device, self.connection)
        self.ctap = Ctap2(self.device)
        return self


@pytest.fixture
def emulator(binary, tmp_path):
    script = tmp_path / "module.events"
    script.write_text("match 0 40 0\n")
    with socket.socket() as reservation:
        reservation.bind(("127.0.0.1", 0))
        port = reservation.getsockname()[1]
    emu = FingerprintEmulator(binary, tmp_path / "session.store", [
        "--ccid-port", str(port), "--fingerprint-script", str(script), "--fingerprint-init",
    ])
    emu.script = script
    emu.ccid_port = port
    try:
        emu.start()
        yield emu
    finally:
        emu.stop()
