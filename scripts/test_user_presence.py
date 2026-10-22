# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Exercise the physical-presence instrument over timed HID reports, without USB."""

import importlib.util
import pathlib
import sys
import types

import pytest

ROOT = pathlib.Path(__file__).resolve().parents[1]
CID = b"\x11\x22\x33\x44"


def frame(command, status, cid=CID):
    return (cid + bytes([command, 0, 1, status])).ljust(64, b"\x00")


class Device:
    def __init__(self, scenarios=(), keepalives=False):
        self.now = 0.0
        self.scenarios = iter(scenarios)
        self.keepalives = keepalives
        self.reports = []
        self.writes = []
        self.closed = False

    def write(self, payload):
        self.writes.append((self.now, payload))
        if payload[4] == 0x86:
            body = payload[7:15] + CID + bytes(5)
            self.reports.append((self.now, (payload[:4] + b"\x86\x00\x11" + body).ljust(64, b"\x00")))
        elif payload[4] == 0x90:
            self.reports.extend((self.now + delay, report)
                                for delay, report in next(self.scenarios))
        elif payload[4] == 0x91:
            self.reports.append((self.now + 0.1, frame(0x90, 0x2D)))

    def read(self, length, timeout_ms):
        assert length == 64
        until = self.now + timeout_ms / 1000
        self.reports.sort(key=lambda item: item[0])
        if self.reports and self.reports[0][0] <= until:
            self.now, report = self.reports.pop(0)
            return report
        self.now = until
        return frame(0xBB, 2) if self.keepalives else []

    def open_path(self, path):
        assert path == b"synthetic"

    def close(self):
        self.closed = True


@pytest.fixture
def subject(monkeypatch):
    transport = types.ModuleType("ctaphid")
    transport.CTAPHID_CBOR = 0x90
    transport.CTAPHID_INIT = 0x86
    transport.CTAPHID_KEEPALIVE = 0xBB
    transport.REPORT_LEN = 64
    transport.write = lambda dev, payload: dev.write(payload)
    transport.hid = types.SimpleNamespace()
    transport.find = lambda: {"path": b"synthetic", "release_number": 0x09EB}
    monkeypatch.setitem(sys.modules, "ctaphid", transport)
    spec = importlib.util.spec_from_file_location("user_presence", ROOT / "tests/50_user_presence_button.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def clock(monkeypatch, subject, dev):
    monkeypatch.setattr(subject, "time", types.SimpleNamespace(monotonic=lambda: dev.now))


def test_cancel_is_sent_even_when_keepalives_stop_after_upneeded(subject, monkeypatch):
    dev = Device([[(0.1, frame(0xBB, 2))]])
    clock(monkeypatch, subject, dev)
    subject.selection(dev, CID, "cancel", False)
    cancels = [when for when, payload in dev.writes if payload[4] == 0x91]
    assert len(cancels) == 1
    assert 1 <= cancels[0] - 0.1 < 1.2, "a silent device must not postpone CANCEL"
    assert dev.now < 2


def test_confirmation_before_the_operator_prompt_is_refused(subject, monkeypatch):
    dev = Device([[(0.1, frame(0xBB, 2)), (0.2, frame(0x90, 0))]])
    clock(monkeypatch, subject, dev)
    with pytest.raises(AssertionError, match="before.*prompt"):
        subject.selection(dev, CID, "press", False)


@pytest.mark.parametrize("keepalives", [False, True])
def test_a_stuck_worker_cannot_extend_the_host_deadline(subject, monkeypatch, keepalives):
    dev = Device([[]], keepalives=keepalives)
    clock(monkeypatch, subject, dev)
    with pytest.raises(TimeoutError):
        subject.selection(dev, CID, "timeout", False)
    assert 35 <= dev.now < 35.2


def test_a_final_response_after_the_host_deadline_is_refused(subject, monkeypatch):
    dev = Device([[(0.06, frame(0xBB, 2)), (35.05, frame(0x90, 0x2F))]])
    clock(monkeypatch, subject, dev)
    with pytest.raises(TimeoutError):
        subject.selection(dev, CID, "timeout", False)


@pytest.mark.parametrize("mode,status,delay", [("press", 0, 1.5), ("timeout", 0x2F, 30)])
def test_expected_presence_results_are_accepted(subject, monkeypatch, mode, status, delay):
    dev = Device([[(0.1, frame(0xBB, 2)), (delay, frame(0x90, status))]])
    clock(monkeypatch, subject, dev)
    subject.selection(dev, CID, mode, False)


@pytest.mark.parametrize("reports,mode,message", [
    ([(0.1, frame(0x90, 0))], "press", "without UPNEEDED"),
    ([(0.1, frame(0xBB, 2)), (1.5, frame(0x90, 0))], "cancel", "CTAP status"),
    ([(0.1, frame(0xBB, 2)), (1.5, frame(0x90, 0x2F))], "timeout", "timed out early"),
    ([(0.1, frame(0xBB, 2, b"bad!"))], "press", "another channel"),
])
def test_invalid_presence_results_are_refused(subject, monkeypatch, reports, mode, message):
    dev = Device([reports])
    clock(monkeypatch, subject, dev)
    if mode == "cancel":
        monkeypatch.setattr(subject, "CANCEL_AFTER_S", 2)
    with pytest.raises(AssertionError, match=message):
        subject.selection(dev, CID, mode, False)


def test_touch_mode_repeats_real_requests_and_identifies_the_input(subject, monkeypatch, capsys):
    scenario = [(0.1, frame(0xBB, 2)), (1.5, frame(0x90, 0))]
    dev = Device([scenario, scenario])
    clock(monkeypatch, subject, dev)
    monkeypatch.setattr(subject.hid, "device", lambda: dev, raising=False)
    monkeypatch.setattr(sys, "argv", ["presence", "--input", "touch", "--repeat", "2"])
    subject.main()
    assert sum(payload[4] == 0x90 for _, payload in dev.writes) == 2
    output = capsys.readouterr().out
    assert output.count("touch the sensor now") == 2
    assert "0x09eb" in output
    assert dev.closed


def test_repeated_check_stops_on_failure_and_closes_the_device(subject, monkeypatch):
    dev = Device([[(0.1, frame(0x90, 0))]])
    clock(monkeypatch, subject, dev)
    monkeypatch.setattr(subject.hid, "device", lambda: dev, raising=False)
    monkeypatch.setattr(sys, "argv", ["presence", "--repeat", "2"])
    with pytest.raises(AssertionError, match="without UPNEEDED"):
        subject.main()
    assert sum(payload[4] == 0x90 for _, payload in dev.writes) == 1
    assert dev.closed


@pytest.mark.parametrize("count", ["0", "-1"])
def test_invalid_repeat_count_is_refused_before_device_access(subject, monkeypatch, count):
    monkeypatch.setattr(sys, "argv", ["presence", "--repeat", count])
    with pytest.raises(SystemExit) as error:
        subject.main()
    assert error.value.code == 2
