# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Unexpected process death must not pass the client runner's cleanup."""

import signal

import pytest

from session import STOP_TIMEOUT, Emulator


@pytest.mark.parametrize("exit_signal", [signal.SIGKILL, signal.SIGTERM])
def test_unexpected_emulator_exit_is_refused(binary, tmp_path, exit_signal):
    emu = Emulator(binary, tmp_path / "session.store")
    try:
        emu.start()
        process = emu.process
        connection = emu.connection
        process.send_signal(exit_signal)
        assert process.wait(timeout=STOP_TIMEOUT) == -exit_signal
        with pytest.raises(AssertionError, match=f"unexpected emulator exit: {-exit_signal}"):
            emu.stop()
        assert emu.process is None
        assert connection.sock.fileno() == -1
        assert not emu.reader.is_alive()
        assert process.stdin.closed and process.stderr.closed
        emu.stop()
    finally:
        emu.stop()


def test_premature_zero_exit_is_not_a_normal_stop(binary, tmp_path):
    store = tmp_path / "incompatible.store"
    store.write_bytes(b"incompatible synthetic flash layout")
    emu = Emulator(binary, store)
    try:
        with pytest.raises(AssertionError, match="not an rsk-emu flash image of this geometry"):
            emu.start()
        assert emu.process.wait(timeout=STOP_TIMEOUT) == 0
        with pytest.raises(AssertionError, match="unexpected emulator exit: 0"):
            emu.stop()
        assert emu.process is None
    finally:
        emu.stop()
