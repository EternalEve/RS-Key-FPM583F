# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors

from types import SimpleNamespace

import pytest

from rsk import fingerprint_trace as trace


def page(index=0, retained=3, first=10):
    count = min(trace.RECORDS_PER_PAGE, max(0, retained - index * trace.RECORDS_PER_PAGE))
    result = trace.HEADER.pack(1, count, retained, first, (first + retained) & trace.U32_MASK,
                              1, 2, 1, 0, 8)
    for offset in range(trace.RECORDS_PER_PAGE):
        if offset < count:
            result += trace.RECORD.pack((first + index * trace.RECORDS_PER_PAGE + offset) & trace.U32_MASK,
                                        42, 8, 3, 2, 1, 1)
        else:
            result += bytes(trace.RECORD.size)
    return result


def test_pages_reassemble_chronologically_without_drain_or_control_commands():
    calls = []
    result = trace.read_trace(lambda index: calls.append(index) or page(index))
    assert calls == [0, 1]
    assert [r["sequence"] for r in result["records"]] == [10, 11, 12]
    assert result["live"] == {"level": "high", "phase": "high", "consent": "wait_low",
                              "adapter_stage": "off", "window": 8}
    assert result["records"][0]["event"] == "edge"
    raw = bytearray(page())
    raw[15] = 3
    assert trace.parse_page(raw, 0).live["adapter_stage"] == "UART ready"


@pytest.mark.parametrize("offset,value", [(0, 2), (1, 3), (2, 129), (4, 99),
    (12, 3), (13, 7), (14, 6), (15, 4), (20, 99), (32, 8), (33, 7), (34, 3), (35, 6)])
def test_malformed_metadata_and_unknown_states_fail_explicitly(offset, value):
    raw = bytearray(page())
    raw[offset] = value
    with pytest.raises(ValueError):
        trace.parse_page(raw, 0)


def test_padding_length_page_and_wrap_are_checked():
    raw = bytearray(page(1))
    raw[-1] = 1
    for data, index in [(raw, 1), (b"", 0), (page(), trace.PAGES), (page(), -1)]:
        with pytest.raises(ValueError):
            trace.parse_page(data, index)
    result = trace.read_trace(lambda index: page(index, first=trace.U32_MASK))
    assert [r["sequence"] for r in result["records"]] == [trace.U32_MASK, 0, 1]
    assert trace.read_trace(lambda index: page(index, retained=0))["records"] == []


def test_mixed_generations_are_refused_instead_of_presented_as_a_snapshot():
    with pytest.raises(ValueError, match="changed during read"):
        trace.read_trace(lambda index: page(index, first=10 + index))


def test_cli_uses_only_select_and_trace_pages(monkeypatch, capsys):
    calls = []
    conn = SimpleNamespace(disconnect=lambda: calls.append("disconnect"))
    monkeypatch.setattr(trace.ccid, "connect", lambda **kw: conn if kw == {"exclusive": True} else None)
    monkeypatch.setattr(trace.ccid, "select", lambda c, aid: (calls.append(tuple(aid)) or (b"", 0x90, 0)))
    monkeypatch.setattr(trace.ccid, "transmit", lambda c, command:
                        (calls.append(tuple(command)) or (page(command[2]), 0x90, 0)))
    trace.run(SimpleNamespace(json=True))
    assert calls == [tuple(trace.ccid.VENDOR_AID), (0, 0x16, 0, 0, 0), (0, 0x16, 1, 0, 0), "disconnect"]
    assert '"retained": 3' in capsys.readouterr().out
