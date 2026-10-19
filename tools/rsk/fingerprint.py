# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors

"""Read volatile startup/UART diagnostics without starting the fingerprint module."""

from enum import IntEnum
import json
from typing import NamedTuple

from . import ccid
from .common import die

INS_STATUS = 0x15
STATUS_VERSION = 1
STATUS_LEN = 6


class Stage(IntEnum):
    OFF = 0
    SUPPLY_STARTING = 1
    UART_STARTING = 2
    UART_READY = 3


class UartFault(IntEnum):
    NONE = 0
    RESET = 1
    OVERRUN = 2
    BREAK = 3
    PARITY = 4
    FRAMING = 5
    INACTIVE = 6
    UNKNOWN = 7


class DriverFault(IntEnum):
    NONE = 0
    UNAVAILABLE = 1
    NO_MATCH = 2
    INPUT_TIMEOUT = 3
    TIMEOUT = 4
    DECLINED = 5
    COMMUNICATION = 6


STAGES = {Stage.OFF: "off", Stage.SUPPLY_STARTING: "supply starting",
          Stage.UART_STARTING: "UART starting", Stage.UART_READY: "UART ready"}
UART_FAULTS = {UartFault.NONE: "none", UartFault.RESET: "reset did not complete",
               UartFault.OVERRUN: "overrun", UartFault.BREAK: "break",
               UartFault.PARITY: "parity", UartFault.FRAMING: "framing",
               UartFault.INACTIVE: "UART inactive", UartFault.UNKNOWN: "unknown UART error"}
DRIVER_FAULTS = {DriverFault.NONE: "none", DriverFault.UNAVAILABLE: "unavailable",
                 DriverFault.NO_MATCH: "no match", DriverFault.INPUT_TIMEOUT: "input timeout",
                 DriverFault.TIMEOUT: "timeout", DriverFault.DECLINED: "declined",
                 DriverFault.COMMUNICATION: "communication"}


class Status(NamedTuple):
    stage: Stage
    uart_fault: UartFault
    driver_fault: DriverFault
    command: int


def parse_status(data):
    if len(data) != STATUS_LEN or data[0] != STATUS_VERSION:
        raise ValueError("unsupported fingerprint diagnostic length/version")
    return Status(Stage(data[1]), UartFault(data[2]), DriverFault(data[3]),
                  int.from_bytes(data[4:6], "big"))


def status_fields(status):
    return {"stage": STAGES[status.stage], "uart_fault": UART_FAULTS[status.uart_fault],
            "last_driver_fault": DRIVER_FAULTS[status.driver_fault],
            "failed_command": f"0x{status.command:04x}"}


def register(sub):
    p = sub.add_parser("fingerprint", help="fingerprint startup diagnostics")
    commands = p.add_subparsers(dest="cmd", required=True)
    status = commands.add_parser("status", help="read the volatile diagnostic snapshot (CCID)")
    status.add_argument("--json", action="store_true", help="emit JSON")
    status.set_defaults(func=run)


def run(args):
    conn = ccid.connect(exclusive=True)
    try:
        _, s1, s2 = ccid.select(conn, ccid.VENDOR_AID)
        if (s1, s2) != ccid.SW_OK:
            die(f"vendor SELECT failed: {s1:02X}{s2:02X}")
        data, s1, s2 = ccid.transmit(conn, [0, INS_STATUS, 0, 0, 0])
        if (s1, s2) == (0x6D, 0):
            die("fingerprint diagnostics unavailable on this firmware")
        if (s1, s2) != ccid.SW_OK:
            die(f"fingerprint status failed: {s1:02X}{s2:02X}")
        try:
            fields = status_fields(parse_status(data))
        except ValueError as error:
            die(f"invalid fingerprint diagnostic response: {error}")
        if args.json:
            print(json.dumps(fields))
        else:
            for name, value in fields.items():
                print(f"{name}: {value}")
    finally:
        conn.disconnect()
