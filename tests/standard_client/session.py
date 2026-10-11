# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Owned emulator process and prompt-driven stdin, with no hardware fallback."""

import json
import signal
import socket
import subprocess
import threading
import time
from contextlib import contextmanager
from enum import Enum

from fido2.ctap2 import Ctap2

from transport import HOST, OPERATION_TIMEOUT, connect

APPROVAL_PROMPT = "└─ [Enter] approve · [d] deny"
START_TIMEOUT = 5
STOP_TIMEOUT = 5


class Reply(Enum):
    APPROVE = "approve"
    DENY = "deny"
    WAIT = "wait"


class Emulator:
    def __init__(self, binary, store, extra_args=()):
        self.binary = binary
        self.store = store
        self.extra_args = list(extra_args)
        self.process = None
        self.device = None
        self.condition = threading.Condition()
        self.lines = []
        self.errors = []
        self.reply = None
        self.prompts = 0

    def start(self):
        # Reserve an available loopback port; startup failure never selects another target.
        with socket.socket() as reservation:
            reservation.bind((HOST, 0))
            self.port = reservation.getsockname()[1]
        self.lines = []
        self.errors = []
        self.prompts = 0
        args = [str(self.binary), "--host", HOST, "--fido-port", str(self.port),
                "--ccid-port", "0", "--touch", "--store", str(self.store)]
        args.extend(self.extra_args)
        self.process = subprocess.Popen(
            args, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE, text=True, bufsize=1,
        )
        self.reader = threading.Thread(target=self._read_stderr)
        self.reader.start()
        self.wait_for("emu: device ready")
        self.device, self.connection = connect(self.port)
        with self.connection.operation():
            self.ctap = Ctap2(self.device)
        print("session " + json.dumps({"pid": self.process.pid, "args": args}, ensure_ascii=False))
        return self

    def _read_stderr(self):
        for line in self.process.stderr:
            with self.condition:
                self.lines.append(line.rstrip())
                if APPROVAL_PROMPT in line:
                    self.prompts += 1
                    if self.reply is None:
                        self.errors.append("confirmation requested outside an armed operation")
                    elif self.reply != Reply.WAIT:
                        answer = "d\n" if self.reply == Reply.DENY else "\n"
                        try:
                            self.process.stdin.write(answer)
                            self.process.stdin.flush()
                        except OSError as error:
                            self.errors.append(f"confirmation input failed: {type(error).__name__}")
                self.condition.notify_all()

    def wait_for(self, text, timeout=START_TIMEOUT):
        deadline = time.monotonic() + timeout
        with self.condition:
            while not any(text in line for line in self.lines):
                remaining = deadline - time.monotonic()
                if remaining <= 0 or self.process.poll() is not None:
                    raise AssertionError(f"emulator did not report {text!r}: {self.lines}")
                self.condition.wait(timeout=min(remaining, 0.1))

    @contextmanager
    def confirmations(self, reply=Reply.APPROVE, timeout=OPERATION_TIMEOUT):
        with self.condition:
            if self.reply is not None:
                raise RuntimeError("confirmation policy already armed")
            self.reply = reply
        try:
            with self.connection.operation(timeout):
                yield
        finally:
            with self.condition:
                self.reply = None
            self.check_errors()

    def check_errors(self):
        assert not self.errors, self.errors
        assert not any("cannot write the flash image" in line for line in self.lines), self.lines

    def check_store(self):
        self.check_errors()
        assert self.store.is_file(), "persistent image was not created"
        print("store " + json.dumps({"path": str(self.store), "bytes": self.store.stat().st_size}))

    def stop(self, expected_returncode=None):
        if self.device is not None:
            self.device.close()
            self.device = None
        if self.process is not None:
            process = self.process
            was_running = process.poll() is None
            if was_running:
                process.terminate()
            try:
                returncode = process.wait(timeout=STOP_TIMEOUT)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=STOP_TIMEOUT)
                raise AssertionError("emulator required a forced kill")
            finally:
                self.reader.join(timeout=STOP_TIMEOUT)
                process.stdin.close()
                process.stderr.close()
                self.process = None
            assert not self.reader.is_alive(), "stderr reader did not stop"
            if expected_returncode is None:
                assert was_running, f"unexpected emulator exit: {returncode}"
                expected_returncode = -signal.SIGTERM
            else:
                assert not was_running, "expected emulator exit has not happened"
            assert returncode == expected_returncode, (
                f"unexpected emulator exit: {returncode} (expected {expected_returncode})"
            )
            self.check_errors()

    def restart(self):
        self.check_store()
        self.stop()
        self.start()
