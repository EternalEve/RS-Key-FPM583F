# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Real SDK requests, stored credentials, and RP verification below scripted input."""

import hashlib
import secrets
import threading
import struct
import time

import pytest
from fido2.client import UserInteraction
from fido2.ctap import CtapError
from fido2.ctap2 import Ctap2
from fido2.ctap2.credman import CredentialManagement
from fido2.ctap2.pin import ClientPin, PinProtocolV1, PinProtocolV2
from fido2.hid import STATUS
from fido2.hid import CTAPHID

from ccid import Card
from ceremony import RK, UV, RelyingParty
from session import Reply
from session import Emulator
from ceremony import set_pin

PERM = ClientPin.PERMISSION


class Finger(UserInteraction):
    def __init__(self):
        self.uv_requests = 0

    def request_uv(self, permissions, rp_id):
        self.uv_requests += 1
        return True

    def request_pin(self, permissions, rp_id):
        raise AssertionError("fingerprint path must not silently fall back to PIN")


def setup(emu):
    pin = secrets.token_hex(6)
    with emu.confirmations():
        ClientPin(emu.ctap).set_pin(pin)
        emu.ctap = Ctap2(emu.device)
    assert emu.ctap.info.options["uv"] is True
    assert "bioEnroll" not in emu.ctap.info.options
    assert ClientPin(emu.ctap).get_uv_retries() == 8
    emu.extra_args.remove("--fingerprint-init")
    return pin


def starts(emu):
    return sum("fingerprint: start " in line for line in emu.lines)


def register(emu, rp=None):
    rp = rp or RelyingParty()
    options, state = rp.server.register_begin(
        rp.user, resident_key_requirement=RK.REQUIRED, user_verification=UV.REQUIRED
    )
    finger = Finger()
    before = starts(emu), emu.prompts
    client = rp.client(emu, finger)
    with emu.confirmations(Reply.WAIT):
        response = client.make_credential(options.public_key)
    data = rp.server.register_complete(state, response)
    assert data.is_user_verified() and data.is_user_present()
    rp.credential = data.credential_data
    assert finger.uv_requests == 1
    assert (starts(emu), emu.prompts) == (before[0] + 1, before[1])
    emu.wait_for("fingerprint: idle")
    return rp


def assert_signed(rp, assertion, digest):
    assert assertion.credential["id"] == rp.credential.credential_id
    assert assertion.auth_data.rp_id_hash == hashlib.sha256(rp.id.encode()).digest()
    assert assertion.auth_data.is_user_present() and assertion.auth_data.is_user_verified()
    rp.credential.public_key.verify(bytes(assertion.auth_data) + digest, assertion.signature)


def assertion(emu, ctap, rp, mode, pin):
    digest = secrets.token_bytes(32)
    allow = [{"type": "public-key", "id": rp.credential.credential_id}]
    kwargs = {"options": {"uv": True}}
    if mode != "direct":
        cp = ClientPin(ctap)
        if mode == "uv-token":
            token = cp.get_uv_token(PERM.GET_ASSERTION, rp.id)
        else:
            token = cp.get_pin_token(pin, PERM.GET_ASSERTION, rp.id)
        kwargs = {"pin_uv_param": cp.protocol.authenticate(token, digest),
                  "pin_uv_protocol": cp.protocol.VERSION}
    return ctap.get_assertion(rp.id, digest, allow, **kwargs), digest


def refused(code, call):
    with pytest.raises(CtapError) as caught:
        call()
    assert caught.value.code == code


@pytest.mark.parametrize("entry", ["hid", "ccid"])
@pytest.mark.parametrize("mode", ["direct", "uv-token", "pin-token"])
def test_three_paths_preserve_registered_public_key(emulator, entry, mode):
    pin = setup(emulator)
    rp = register(emulator)
    card = Card(emulator.ccid_port) if entry == "ccid" else None
    try:
        ctap = Ctap2(card) if card else emulator.ctap
        before = starts(emulator), emulator.prompts
        with emulator.confirmations(Reply.APPROVE if mode == "pin-token" else Reply.WAIT):
            result, digest = assertion(emulator, ctap, rp, mode, pin)
        assert_signed(rp, result, digest)
        assert starts(emulator) - before[0] == (0 if mode == "pin-token" else 1)
        assert emulator.prompts - before[1] == (1 if mode == "pin-token" else 0)
        assert ClientPin(ctap).get_uv_retries() == 8
    finally:
        if card:
            card.close()


@pytest.mark.parametrize("entry", ["hid", "ccid"])
def test_full_webauthn_client_verifies_fingerprint_at_rp(emulator, entry):
    setup(emulator)
    card = Card(emulator.ccid_port) if entry == "ccid" else None
    original = emulator.device, emulator.ctap
    try:
        if card:
            emulator.device, emulator.ctap = card, Ctap2(card)
        rp = register(emulator)
        options, state = rp.request()
        finger = Finger()
        before = starts(emulator), emulator.prompts
        client = rp.client(emulator, finger)
        with emulator.confirmations(Reply.WAIT):
            response = client.get_assertion(options.public_key).get_response(0)
        assert rp.server.authenticate_complete(state, [rp.credential], response) == rp.credential
        assert response.response.authenticator_data.is_user_verified()
        assert response.response.user_handle == rp.user.id
        assert finger.uv_requests == 1
        assert (starts(emulator), emulator.prompts) == (before[0] + 1, before[1])
    finally:
        emulator.device, emulator.ctap = original
        if card:
            card.close()


@pytest.mark.parametrize("origin", ["hid", "ccid"])
@pytest.mark.parametrize("mode", ["uv-token", "pin-token"])
def test_valid_token_keeps_its_permission_and_rp_rules_across_entries(emulator, origin, mode):
    pin = setup(emulator)
    rp = register(emulator)
    with Card(emulator.ccid_port) as card:
        ccid = Ctap2(card)
        source, target = (emulator.ctap, ccid) if origin == "hid" else (ccid, emulator.ctap)
        cp = ClientPin(source)
        before = starts(emulator), emulator.prompts
        token = cp.get_uv_token(PERM.GET_ASSERTION, rp.id) if mode == "uv-token" else (
            cp.get_pin_token(pin, PERM.GET_ASSERTION, rp.id))
        digest = secrets.token_bytes(32)
        with emulator.confirmations(Reply.WAIT if mode == "uv-token" else Reply.APPROVE):
            answer = target.get_assertion(rp.id, digest,
                [{"type": "public-key", "id": rp.credential.credential_id}],
                pin_uv_param=cp.protocol.authenticate(token, digest), pin_uv_protocol=cp.protocol.VERSION)
        assert_signed(rp, answer, digest)
        assert (starts(emulator) - before[0], emulator.prompts - before[1]) == (
            (1, 0) if mode == "uv-token" else (0, 1))


@pytest.mark.parametrize("entry", ["hid", "ccid"])
def test_fingerprint_token_presence_cannot_approve_a_second_assertion(emulator, entry):
    setup(emulator)
    rp = register(emulator)
    card = Card(emulator.ccid_port) if entry == "ccid" else None
    try:
        ctap = Ctap2(card) if card else emulator.ctap
        cp = ClientPin(ctap)
        token = cp.get_uv_token(PERM.GET_ASSERTION, rp.id)
        before = starts(emulator), emulator.prompts

        def request():
            digest = secrets.token_bytes(32)
            return ctap.get_assertion(rp.id, digest,
                [{"type": "public-key", "id": rp.credential.credential_id}],
                pin_uv_param=cp.protocol.authenticate(token, digest), pin_uv_protocol=cp.protocol.VERSION), digest

        with emulator.confirmations(Reply.WAIT):
            answer, digest = request()
            assert_signed(rp, answer, digest)
            refused(CtapError.ERR.PIN_AUTH_INVALID, request)
        assert (starts(emulator), emulator.prompts) == before
    finally:
        if card:
            card.close()


@pytest.mark.parametrize("protocol", [PinProtocolV1, PinProtocolV2])
@pytest.mark.parametrize("event", ["nomatch", "unknown", "stale", "generation"])
def test_invalid_results_never_issue_uv_token(emulator, protocol, event):
    setup(emulator)
    emulator.script.write_text(f"{event} 0 40 0\n")
    cp = ClientPin(emulator.ctap, protocol())
    refused(CtapError.ERR.UV_INVALID, lambda: cp.get_uv_token(PERM.GET_ASSERTION, "example.com"))
    assert cp.get_uv_retries() == 7
    assert emulator.prompts == 0


def test_direct_uv_failure_has_no_up_only_fallback(emulator):
    pin = setup(emulator)
    rp = register(emulator)
    emulator.script.write_text("nomatch 0 40 0\n")
    refused(CtapError.ERR.PUAT_REQUIRED, lambda: assertion(emulator, emulator.ctap, rp, "direct", pin))
    assert ClientPin(emulator.ctap).get_uv_retries() == 7


def test_up_only_and_silent_probe_never_start_matching(emulator):
    pin = setup(emulator)
    emulator.script.write_text("wait 0 0 30000\n")
    rp = RelyingParty()
    rp.register(emulator, None, resident=RK.DISCOURAGED, uv=UV.DISCOURAGED)
    rp.authenticate(emulator, None, discoverable=False, uv=UV.DISCOURAGED)
    before = emulator.prompts
    with emulator.connection.operation():
        result = emulator.ctap.get_assertion(rp.id, secrets.token_bytes(32),
            [{"type": "public-key", "id": rp.credential.credential_id}], options={"up": False})
    assert not result.auth_data.is_user_present() and not result.auth_data.is_user_verified()
    assert emulator.prompts == before and starts(emulator) == 0
    assert ClientPin(emulator.ctap).get_uv_retries() == 8
    assert pin


@pytest.mark.parametrize("recovery", ["hid", "ccid"])
def test_uv_budget_is_shared_persistent_and_pin_restores_it(emulator, recovery):
    pin = setup(emulator)
    emulator.script.write_text("nomatch 0 40 0\n")
    with Card(emulator.ccid_port) as card:
        ccid = Ctap2(card)
        for remaining in range(8, 0, -1):
            cp = ClientPin(emulator.ctap if remaining % 2 else ccid)
            code = CtapError.ERR.UV_BLOCKED if remaining == 1 else CtapError.ERR.UV_INVALID
            refused(code, lambda: cp.get_uv_token(PERM.GET_ASSERTION, "example.com"))
            assert ClientPin(ccid).get_uv_retries() == remaining - 1
    emulator.stop()
    emulator.start()
    assert ClientPin(emulator.ctap).get_uv_retries() == 0
    refused(CtapError.ERR.UV_BLOCKED, lambda: ClientPin(emulator.ctap).get_uv_token(PERM.GET_ASSERTION, "example.com"))
    with Card(emulator.ccid_port) as card:
        cp = ClientPin(Ctap2(card) if recovery == "ccid" else emulator.ctap)
        cp.get_pin_token(pin, PERM.GET_ASSERTION, "example.com")
        assert cp.get_uv_retries() == 8


def test_fingerprint_does_not_clear_shared_pin_soft_lock(emulator):
    pin = setup(emulator)
    with Card(emulator.ccid_port) as card:
        ccid = Ctap2(card)
        for index in range(3):
            cp = ClientPin(emulator.ctap if index % 2 else ccid)
            code = CtapError.ERR.PIN_AUTH_BLOCKED if index == 2 else CtapError.ERR.PIN_INVALID
            refused(code, lambda: cp.get_pin_token("incorrect-pin", PERM.GET_ASSERTION, "example.com"))
        ClientPin(ccid).get_uv_token(PERM.GET_ASSERTION, "example.com")
        refused(CtapError.ERR.PIN_AUTH_BLOCKED, lambda: ClientPin(emulator.ctap).get_pin_token(pin, PERM.GET_ASSERTION, "example.com"))
        assert ClientPin(emulator.ctap).get_pin_retries()[0] == 5


@pytest.mark.parametrize("processing", [False, True])
def test_cancel_in_input_and_processing_is_final(emulator, processing):
    setup(emulator)
    emulator.script.write_text("match 0 5000 0\n" if processing else "match 5000 40 0\n")
    event = threading.Event()
    statuses = []
    def keepalive(status):
        statuses.append(status)
        if status == (STATUS.PROCESSING if processing else STATUS.UPNEEDED):
            event.set()
    refused(CtapError.ERR.KEEPALIVE_CANCEL, lambda: ClientPin(emulator.ctap).get_uv_token(
        PERM.GET_ASSERTION, "example.com", event=event, on_keepalive=keepalive))
    assert event.is_set() and statuses
    assert ClientPin(emulator.ctap).get_uv_retries() == 7
    emulator.script.write_text("match 0 40 0\n")
    ClientPin(emulator.ctap).get_uv_token(PERM.GET_ASSERTION, "example.com")
    assert ClientPin(emulator.ctap).get_uv_retries() == 8


def test_draining_blocks_other_entry_but_pin_and_button_work(emulator):
    pin = setup(emulator)
    rp = register(emulator)
    emulator.script.write_text("match 0 5000 1500\n")
    event = threading.Event()
    refused(CtapError.ERR.KEEPALIVE_CANCEL, lambda: ClientPin(emulator.ctap).get_uv_token(
        PERM.GET_ASSERTION, rp.id, event=event, on_keepalive=lambda _: event.set()))
    with Card(emulator.ccid_port) as card:
        cp = ClientPin(Ctap2(card))
        refused(CtapError.ERR.OTHER, lambda: cp.get_uv_token(PERM.GET_ASSERTION, rp.id))
        with emulator.confirmations():
            result, digest = assertion(emulator, cp.ctap, rp, "pin-token", pin)
        assert_signed(rp, result, digest)


def test_both_entry_enumerations_keep_their_source(emulator):
    pin = setup(emulator)
    rp = register(emulator)
    register(emulator)
    another = RelyingParty()
    another.id, another.origin, another.server = rp.id, rp.origin, rp.server
    register(emulator, another)
    with Card(emulator.ccid_port) as card:
        ccid = Ctap2(card)
        cp = ClientPin(emulator.ctap)
        token = cp.get_pin_token(pin, PERM.CREDENTIAL_MGMT)
        walks = [CredentialManagement(ctap, cp.protocol, token) for ctap in (emulator.ctap, ccid)]
        for own, other in (walks, walks[::-1]):
            own.enumerate_rps_begin()
            refused(CtapError.ERR.NOT_ALLOWED, other.enumerate_rps_next)
            own.enumerate_rps_next()
        digest = hashlib.sha256(rp.id.encode()).digest()
        for own, other in (walks, walks[::-1]):
            own.enumerate_creds_begin(digest)
            refused(CtapError.ERR.NOT_ALLOWED, other.enumerate_creds_next)
            own.enumerate_creds_next()


def test_old_pin_credential_survives_enabling_fingerprint(emulator, binary, tmp_path):
    store = tmp_path / "phase-two.store"
    legacy = Emulator(binary, store)
    try:
        legacy.start()
        pin = set_pin(legacy)
        rp = RelyingParty()
        rp.register(legacy, pin)
        old = rp.credential
        legacy.stop()
        upgraded = type(emulator)(binary, store, emulator.extra_args + ["--ccid-port", "0"])
        try:
            upgraded.start()
            for mode in ("pin-token", "uv-token", "direct"):
                with upgraded.confirmations(Reply.APPROVE if mode == "pin-token" else Reply.WAIT):
                    result, digest = assertion(upgraded, upgraded.ctap, rp, mode, pin)
                assert_signed(rp, result, digest)
                assert rp.credential == old
            upgraded.check_store()
        finally:
            upgraded.stop()
    finally:
        legacy.stop()


@pytest.mark.parametrize("kind", ["permission", "rp", "authentication"])
def test_fingerprint_token_cannot_bypass_scope_or_authentication(emulator, kind):
    setup(emulator)
    rp = register(emulator)
    cp = ClientPin(emulator.ctap)
    token = cp.get_uv_token(PERM.MAKE_CREDENTIAL if kind == "permission" else PERM.GET_ASSERTION, rp.id)
    digest = secrets.token_bytes(32)
    mac = cp.protocol.authenticate(token, digest)
    if kind == "authentication":
        mac = bytes([mac[0] ^ 1]) + mac[1:]
    before = starts(emulator), emulator.prompts
    code = CtapError.ERR.PIN_AUTH_INVALID
    refused(code, lambda: emulator.ctap.get_assertion(
        "another.example.com" if kind == "rp" else rp.id, digest,
        [{"type": "public-key", "id": rp.credential.credential_id}],
        pin_uv_param=mac, pin_uv_protocol=cp.protocol.VERSION))
    assert (starts(emulator), emulator.prompts) == before
    if kind == "permission":
        with emulator.confirmations(Reply.WAIT):
            control = emulator.ctap.make_credential(digest, {"id": rp.id},
                {"id": secrets.token_bytes(16), "name": "permission-control"},
                [{"type": "public-key", "alg": -7}], pin_uv_param=mac,
                pin_uv_protocol=cp.protocol.VERSION)
        assert control.auth_data.is_user_verified()


def test_pin_hard_block_disables_uv_across_restart(emulator):
    setup(emulator)
    for index in range(8):
        if index and index % 3 == 0:
            emulator.stop()
            emulator.start()
        code = CtapError.ERR.PIN_BLOCKED if index == 7 else (
            CtapError.ERR.PIN_AUTH_BLOCKED if index % 3 == 2 else CtapError.ERR.PIN_INVALID)
        refused(code, lambda: ClientPin(emulator.ctap).get_pin_token("incorrect-pin", PERM.GET_ASSERTION, "example.com"))
    assert ClientPin(emulator.ctap).get_uv_retries() == 0
    before = starts(emulator)
    refused(CtapError.ERR.UV_BLOCKED, lambda: ClientPin(emulator.ctap).get_uv_token(PERM.GET_ASSERTION, "example.com"))
    assert starts(emulator) == before


def test_ccid_keeps_u2f_and_both_apdu_chain_directions(emulator):
    setup(emulator)
    with Card(emulator.ccid_port) as card:
        assert card.call(CTAPHID.MSG, b"\x00\x03\x00\x00") == b"U2F_V2"
        cp = ClientPin(Ctap2(card))
        assert len(cp.get_uv_token(PERM.GET_ASSERTION, "example.com")) == 32


def test_wrong_cid_cancel_does_not_end_current_matching(emulator):
    setup(emulator)
    emulator.script.write_text("match 0 800 0\n")
    seen = []
    def keepalive(status):
        seen.append(status)
        cid = emulator.device.device._channel_id ^ 1
        emulator.connection.write_packet(struct.pack(">IBH", cid, 0x80 | CTAPHID.CANCEL, 0).ljust(64, b"\x00"))
    token = ClientPin(emulator.ctap).get_uv_token(PERM.GET_ASSERTION, "example.com", on_keepalive=keepalive)
    assert len(token) == 32 and seen == [STATUS.PROCESSING]
    assert ClientPin(emulator.ctap).get_uv_retries() == 8


def test_partial_next_report_survives_end_of_matching(emulator):
    setup(emulator)
    emulator.script.write_text("match 0 1000 0\n")
    cid = emulator.device.device._channel_id
    nonce = secrets.token_bytes(16)
    ping = (struct.pack(">IBH", cid, 0x80 | CTAPHID.PING, len(nonce)) + nonce).ljust(64, b"\x00")
    sent = []
    def keepalive(status):
        assert status == STATUS.PROCESSING
        emulator.connection.sock.sendall(ping[:30])
        sent.append(True)
    ClientPin(emulator.ctap).get_uv_token(PERM.GET_ASSERTION, "example.com", on_keepalive=keepalive)
    assert sent == [True]
    with emulator.connection.operation():
        emulator.connection.sock.sendall(ping[30:])
        response = emulator.connection.read_packet()
    assert response[:23] == ping[:23]
    assert emulator.ctap.get_info().options["uv"] is True


def test_same_cid_init_aborts_matching_and_drains_the_old_reply(emulator):
    setup(emulator)
    emulator.script.write_text("match 0 1000 0\n")
    cid = emulator.device.device._channel_id
    nonce = secrets.token_bytes(8)
    request = (struct.pack(">IBH", cid, 0x80 | CTAPHID.INIT, len(nonce)) + nonce).ljust(64, b"\x00")
    replies = []
    read = emulator.connection.read_packet

    def capture():
        packet = read()
        if packet[4] == 0x80 | CTAPHID.INIT:
            replies.append(packet)
        return packet

    def keepalive(status):
        assert status == STATUS.PROCESSING
        emulator.connection.write_packet(request)

    emulator.connection.read_packet = capture
    try:
        refused(CtapError.ERR.INVALID_COMMAND, lambda: ClientPin(emulator.ctap).get_uv_token(
            PERM.GET_ASSERTION, "example.com", on_keepalive=keepalive))
    finally:
        emulator.connection.read_packet = read
    assert len(replies) == 1
    response = replies[0]
    assert response[:7] == struct.pack(">IBH", cid, 0x80 | CTAPHID.INIT, 17)
    assert response[7:15] == nonce and response[15:19] == struct.pack(">I", cid)
    assert emulator.ctap.get_info().options["uv"] is True
    assert ClientPin(emulator.ctap).get_uv_retries() == 7
    emulator.wait_for("fingerprint: idle")
    assert len(ClientPin(emulator.ctap).get_uv_token(PERM.GET_ASSERTION, "example.com")) == 32


def test_hid_cancel_cannot_cancel_ccid_matching(emulator):
    setup(emulator)
    rp = register(emulator)
    emulator.script.write_text("match 0 700 0\n")
    with Card(emulator.ccid_port) as card:
        cp = ClientPin(Ctap2(card))
        result, failures = [], []
        def run():
            try:
                result.append(cp.get_uv_token(PERM.GET_ASSERTION, rp.id))
            except Exception as error:
                failures.append(error)
        worker = threading.Thread(target=run)
        before = len(emulator.lines)
        worker.start()
        with emulator.condition:
            deadline = time.monotonic() + 5
            while not any("fingerprint: matching" in line for line in emulator.lines[before:]):
                left = deadline - time.monotonic()
                assert left > 0, "CCID never started matching"
                emulator.condition.wait(timeout=left)
        with emulator.connection.operation():
            cid = emulator.device.device._channel_id
            emulator.connection.write_packet(struct.pack(">IBH", cid, 0x80 | CTAPHID.CANCEL, 0).ljust(64, b"\x00"))
        worker.join(timeout=5)
        assert not worker.is_alive() and not failures and len(result) == 1
        digest = secrets.token_bytes(32)
        answer = cp.ctap.get_assertion(rp.id, digest,
            [{"type": "public-key", "id": rp.credential.credential_id}],
            pin_uv_param=cp.protocol.authenticate(result[0], digest), pin_uv_protocol=cp.protocol.VERSION)
        assert_signed(rp, answer, digest)


def test_known_input_timeout_refunds_but_communication_does_not(emulator):
    setup(emulator)
    cp = ClientPin(emulator.ctap)
    emulator.script.write_text("input-timeout 40 0 0\n")
    refused(CtapError.ERR.USER_ACTION_TIMEOUT, lambda: cp.get_uv_token(PERM.GET_ASSERTION, "example.com"))
    assert cp.get_uv_retries() == 8
    emulator.script.write_text("communication 0 40 0\n")
    refused(CtapError.ERR.OTHER, lambda: cp.get_uv_token(PERM.GET_ASSERTION, "example.com"))
    assert cp.get_uv_retries() == 7
