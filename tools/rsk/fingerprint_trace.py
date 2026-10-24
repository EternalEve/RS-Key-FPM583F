# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors

"""Read bounded touch observations from an explicit measurement image."""

from enum import IntEnum
import json
import struct
from typing import NamedTuple

from . import ccid
from .common import die

INS_TRACE = 0x16
VERSION = 1
CAPACITY = 128
RECORDS_PER_PAGE = 2
HEADER = struct.Struct("<BBHII4BI")
RECORD = struct.Struct("<III4B")
PAGE_LEN = HEADER.size + RECORDS_PER_PAGE * RECORD.size
PAGES = CAPACITY // RECORDS_PER_PAGE
U32_MASK = (1 << 32) - 1


class Phase(IntEnum):
    COLD = 0
    READY = 1
    HIGH = 2
    PENDING = 3
    RUNNING = 4
    SETTLING = 5
    DISABLED = 6


class Consent(IntEnum):
    CLOSED = 0
    WAIT_LOW = 1
    ARMED = 2
    EVENT = 3
    SPENT = 4
    DISABLED = 5


class Level(IntEnum):
    LOW = 0
    HIGH = 1
    UNKNOWN = 2


class Event(IntEnum):
    INITIALIZED = 1
    RESAMPLE = 2
    EDGE = 3
    OPEN = 4
    CLOSE = 5
    TAKEN = 6
    SERVICE = 7


class Observation(NamedTuple):
    sequence: int
    ms: int
    window: int
    event: Event
    phase: Phase
    level: Level
    consent: Consent


class Page(NamedTuple):
    retained: int
    first_sequence: int
    next_sequence: int
    live: dict
    records: tuple


def parse_page(data, page):
    from .fingerprint import STAGES, Stage

    if len(data) != PAGE_LEN or not 0 <= page < PAGES:
        raise ValueError("unsupported touch trace length/page")
    version, count, retained, first, next_seq, level, phase, consent, stage, window = HEADER.unpack_from(data)
    expected = min(RECORDS_PER_PAGE, max(0, retained - page * RECORDS_PER_PAGE))
    if (version != VERSION or retained > CAPACITY or count != expected
            or ((next_seq - first) & U32_MASK) != retained):
        raise ValueError("invalid touch trace version/count/sequence bounds")
    live = {"level": Level(level).name.lower(), "phase": Phase(phase).name.lower(),
            "consent": Consent(consent).name.lower(), "adapter_stage": STAGES[Stage(stage)],
            "window": window}
    records = []
    for index in range(count):
        seq, ms, window, event, phase, level, consent = RECORD.unpack_from(data, HEADER.size + index * RECORD.size)
        if seq != ((first + page * RECORDS_PER_PAGE + index) & U32_MASK):
            raise ValueError("touch trace record sequence mismatch")
        records.append(Observation(seq, ms, window, Event(event), Phase(phase), Level(level), Consent(consent)))
    if any(data[HEADER.size + count * RECORD.size:]):
        raise ValueError("nonzero touch trace padding")
    return Page(retained, first, next_seq, live, tuple(records))


def read_trace(exchange):
    first = parse_page(exchange(0), 0)
    records = list(first.records)
    for index in range(1, (first.retained + RECORDS_PER_PAGE - 1) // RECORDS_PER_PAGE):
        page = parse_page(exchange(index), index)
        if page[:3] != first[:3]:
            raise ValueError("touch trace changed during read; retry with both inputs idle")
        records.extend(page.records)
    return {"first_sequence": first.first_sequence, "next_sequence": first.next_sequence,
            "retained": first.retained, "live": first.live,
            "records": [{"sequence": r.sequence, "ms": r.ms, "window": r.window,
                         "event": r.event.name.lower(), "phase": r.phase.name.lower(),
                         "level": r.level.name.lower(), "consent": r.consent.name.lower()}
                        for r in records]}


def run(args):
    conn = ccid.connect(exclusive=True)
    try:
        _, s1, s2 = ccid.select(conn, ccid.VENDOR_AID)
        if (s1, s2) != ccid.SW_OK:
            die(f"vendor SELECT failed: {s1:02X}{s2:02X}")

        def exchange(page):
            data, s1, s2 = ccid.transmit(conn, [0, INS_TRACE, page, 0, 0])
            if (s1, s2) == (0x6D, 0):
                die("touch trace requires a fingerprint-touch-trace measurement image")
            if (s1, s2) != ccid.SW_OK:
                die(f"touch trace failed: {s1:02X}{s2:02X}")
            return bytes(data)

        try:
            result = read_trace(exchange)
        except ValueError as error:
            die(f"invalid touch trace: {error}")
        if args.json:
            print(json.dumps(result))
        else:
            print(f"live: {result['live']}")
            for record in result["records"]:
                print(f"{record['sequence']:>10} {record['ms']:>10}ms "
                      f"{record['event']:<11} {record['level']:<7} {record['phase']:<9} "
                      f"{record['consent']:<8} window={record['window']}")
    finally:
        conn.disconnect()
