# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""CCID/TCP and APDU framing only; python-fido2 owns CTAP and authentication."""

import socket
import struct
import time

from fido2.ctap import CtapDevice
from fido2.hid import CAPABILITY, CTAPHID

FIDO_AID = bytes.fromhex("a0000006472f0001")
TIMEOUT = 10


class Card(CtapDevice):
    capabilities = CAPABILITY.CBOR | CAPABILITY.NMSG

    def __init__(self, port):
        self.socket = socket.create_connection(("127.0.0.1", port), TIMEOUT)
        self.seq = 0
        self.power_on()

    @classmethod
    def list_devices(cls):
        raise AssertionError("test transport never enumerates hardware")

    def close(self):
        self.socket.close()

    def read(self, size, deadline):
        data = bytearray()
        while len(data) < size:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("CCID deadline expired")
            self.socket.settimeout(remaining)
            part = self.socket.recv(size - len(data))
            if not part:
                raise EOFError("CCID connection closed")
            data.extend(part)
        return bytes(data)

    def message(self, kind, body=b""):
        deadline = time.monotonic() + TIMEOUT
        self.seq = (self.seq + 1) & 0xff
        header = bytes([kind]) + struct.pack("<I", len(body)) + bytes([0, self.seq, 0, 0, 0])
        payload = header + body
        self.socket.sendall(b"\x00" + struct.pack(">I", len(payload)) + payload)
        while True:
            size = struct.unpack(">I", self.read(4, deadline))[0]
            assert 10 <= size <= 8192, "invalid CCID length"
            response = self.read(size, deadline)
            assert response[5:7] == bytes([0, self.seq])
            assert struct.unpack("<I", response[1:5])[0] == size - 10
            if response[7] & 0xc0 == 0x80:
                continue
            assert response[7] & 0xc0 == 0, f"CCID error {response[7:9].hex()}"
            return response[10:]

    def apdu(self, body):
        response = self.message(0x6f, body)
        data = bytearray()
        for _ in range(64):
            assert len(response) >= 2
            data.extend(response[:-2])
            status = response[-2:]
            if status[0] != 0x61:
                assert status == b"\x90\x00", f"APDU status {status.hex()}"
                return bytes(data)
            response = self.message(0x6f, b"\x00\xc0\x00\x00\x00")
        raise AssertionError("unbounded response chain")

    def power_on(self):
        self.message(0x62)
        assert self.apdu(b"\x00\xa4\x04\x00" + bytes([len(FIDO_AID)]) + FIDO_AID) == b"U2F_V2"

    def call(self, cmd, data=b"", event=None, on_keepalive=None):
        if cmd == CTAPHID.CBOR:
            while len(data) > 96:
                assert self.apdu(b"\x90\x10\x00\x00\x60" + data[:96]) == b""
                data = data[96:]
            return self.apdu(b"\x80\x10\x00\x00" + bytes([len(data)]) + data)
        if cmd == CTAPHID.MSG:
            return self.apdu(data)
        raise AssertionError(f"unsupported transport command {cmd}")
