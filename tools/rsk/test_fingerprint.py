# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors

from types import SimpleNamespace

import pytest

from rsk import fingerprint


def test_status_decodes_uart_break_and_failed_count_without_template_data():
    status = fingerprint.parse_status(bytes([1, 3, 3, 6, 2, 3]))
    assert status == fingerprint.Status(fingerprint.Stage.UART_READY,
        fingerprint.UartFault.BREAK, fingerprint.DriverFault.COMMUNICATION, 0x0203)
    assert fingerprint.status_fields(status) == {"stage": "UART ready", "uart_fault": "break",
        "last_driver_fault": "communication", "failed_command": "0x0203"}


@pytest.mark.parametrize("data", [b"", bytes(5), bytes(7), bytes([2, 0, 0, 0, 0, 0]),
    bytes([1, 4, 0, 0, 0, 0]), bytes([1, 0, 8, 0, 0, 0]), bytes([1, 0, 0, 7, 0, 0])])
def test_unknown_status_never_becomes_healthy(data):
    with pytest.raises(ValueError):
        fingerprint.parse_status(data)


def test_status_sends_only_vendor_select_and_snapshot(monkeypatch, capsys):
    calls = []
    conn = SimpleNamespace(disconnect=lambda: calls.append("disconnect"))
    monkeypatch.setattr(fingerprint.ccid, "connect", lambda **kw: conn if kw == {"exclusive": True} else None)
    monkeypatch.setattr(fingerprint.ccid, "select", lambda c, aid: (calls.append(tuple(aid)) or (b"", 0x90, 0)))
    monkeypatch.setattr(fingerprint.ccid, "transmit", lambda c, data: (calls.append(tuple(data)) or (bytes([1, 0, 0, 0, 0, 0]), 0x90, 0)))
    fingerprint.run(SimpleNamespace(json=True))
    assert calls == [tuple(fingerprint.ccid.VENDOR_AID), (0, 0x15, 0, 0, 0), "disconnect"]
    assert '"uart_fault": "none"' in capsys.readouterr().out
