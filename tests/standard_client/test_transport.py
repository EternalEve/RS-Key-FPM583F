# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Stream integrity and bounded failures, independent of an authenticator."""

import socket
import threading
import time

import pytest

from transport import REPORT_SIZE, SocketConnection


@pytest.fixture
def peers():
    local, peer = socket.socketpair()
    connection = SocketConnection(local)
    try:
        with connection.operation(timeout=1):
            yield connection, peer
    finally:
        connection.close()
        peer.close()


def test_fragmented_and_coalesced_reports(peers):
    connection, peer = peers
    first = bytes(range(REPORT_SIZE))
    second = bytes(reversed(first))

    def send():
        peer.sendall(first[:13])
        time.sleep(0.02)
        peer.sendall(first[13:] + second)

    writer = threading.Thread(target=send)
    writer.start()
    try:
        assert connection.read_packet() == first
        assert connection.read_packet() == second
    finally:
        writer.join(timeout=1)
    assert not writer.is_alive()


@pytest.mark.parametrize("length", [0, 13])
def test_eof_preserves_partial_report_cause_and_closes(peers, length):
    connection, peer = peers
    peer.sendall(b"x" * length)
    peer.shutdown(socket.SHUT_WR)
    with pytest.raises(ConnectionError, match=f"EOF after {length} report bytes"):
        connection.read_packet()
    assert connection.sock.fileno() == -1


@pytest.mark.parametrize("length", [0, 63, 65])
def test_invalid_output_is_never_padded_or_truncated(peers, length):
    connection, peer = peers
    with pytest.raises(ValueError, match="exactly 64 bytes"):
        connection.write_packet(b"x" * length)
    connection.write_packet(b"v" * REPORT_SIZE)
    assert peer.recv(REPORT_SIZE) == b"v" * REPORT_SIZE


def test_partial_report_timeout_invalidates_connection(peers):
    connection, peer = peers
    connection.deadline = time.monotonic() + 0.03
    peer.sendall(b"x" * 13)
    with pytest.raises(TimeoutError):
        connection.read_packet()
    assert connection.sock.fileno() == -1


def test_deadline_does_not_restart_after_progress(peers):
    connection, peer = peers
    connection.deadline = time.monotonic() + 0.03
    peer.sendall(b"x" * REPORT_SIZE)
    assert connection.read_packet() == b"x" * REPORT_SIZE
    time.sleep(0.04)
    with pytest.raises(TimeoutError, match="operation deadline expired"):
        connection.read_packet()
    assert connection.sock.fileno() == -1


def test_close_unblocks_a_waiting_read(peers):
    connection, _ = peers
    started = threading.Event()
    errors = []

    def read():
        started.set()
        try:
            connection.read_packet()
        except OSError as error:
            errors.append(error)

    reader = threading.Thread(target=read)
    reader.start()
    assert started.wait(timeout=1)
    connection.close()
    reader.join(timeout=1)
    assert not reader.is_alive()
    assert len(errors) == 1
