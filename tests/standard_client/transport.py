# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""A report-only connection for python-fido2; never enumerates USB devices."""

import socket
import time
from contextlib import contextmanager

from fido2.hid import CtapHidDevice
from fido2.hid.base import CtapHidConnection, HidDescriptor

REPORT_SIZE = 64
CONNECT_TIMEOUT = 5
OPERATION_TIMEOUT = 10
HOST = "127.0.0.1"


class SocketConnection(CtapHidConnection):
    def __init__(self, sock):
        self.sock = sock
        self.deadline = None

    @contextmanager
    def operation(self, timeout=OPERATION_TIMEOUT):
        if self.deadline is not None:
            raise RuntimeError("one operation per connection")
        self.deadline = time.monotonic() + timeout
        try:
            yield
        finally:
            self.deadline = None

    def _set_timeout(self):
        if self.deadline is None:
            raise RuntimeError("an operation deadline is required")
        remaining = self.deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError("CTAPHID operation deadline expired")
        self.sock.settimeout(remaining)

    def read_packet(self):
        packet = bytearray()
        try:
            while len(packet) < REPORT_SIZE:
                self._set_timeout()
                part = self.sock.recv(REPORT_SIZE - len(packet))
                if not part:
                    raise ConnectionError(f"CTAPHID EOF after {len(packet)} report bytes")
                packet.extend(part)
        except OSError:
            # A partial or late response must never become the next operation's result.
            self.close()
            raise
        return bytes(packet)

    def write_packet(self, data):
        if len(data) != REPORT_SIZE:
            raise ValueError("CTAPHID reports must be exactly 64 bytes, without report ID")
        try:
            self._set_timeout()
            self.sock.sendall(data)
        except OSError:
            self.close()
            raise

    def close(self):
        try:
            self.sock.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass
        self.sock.close()


def connect(port):
    connection = SocketConnection(socket.create_connection((HOST, port), timeout=CONNECT_TIMEOUT))
    descriptor = HidDescriptor(
        path=f"tcp://{HOST}:{port}/rsk-emu",
        vid=0x1209,
        pid=0x0001,
        report_size_in=REPORT_SIZE,
        report_size_out=REPORT_SIZE,
        product_name="RS-Key software emulator (TCP)",
        serial_number="RSKEMU0001",
    )
    try:
        with connection.operation():
            device = CtapHidDevice(descriptor, connection)
    except BaseException:
        connection.close()
        raise
    return device, connection
