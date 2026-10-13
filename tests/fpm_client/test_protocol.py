# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Standard BioEnrollment and RP verification through actual FPM wire parsing."""

import secrets
import threading
import time

import pytest
from fido2.ctap import CtapError
from fido2.ctap2 import Ctap2, FPBioEnrollment, CaptureError
from fido2.ctap2.pin import ClientPin, PinProtocolV1, PinProtocolV2

from ccid import Card
from ceremony import RelyingParty, set_pin
from session import Reply
from test_authorization import assertion, assert_signed, Finger

PERM = ClientPin.PERMISSION


def calls(emu, command):
    return sum(line == f"fpm-wire: command {command:04x}" for line in emu.lines)


def refused(code, call):
    with pytest.raises(CtapError) as caught:
        call()
    assert caught.value.code == code


def bio(emu, pin, ctap=None, protocol=None):
    ctap = ctap or emu.ctap
    cp = ClientPin(ctap, protocol)
    with emu.confirmations():
        token = cp.get_pin_token(pin, PERM.BIO_ENROLL)
    return FPBioEnrollment(ctap, cp.protocol, token)


def enroll(emu, api):
    context = api.enroll()
    before = calls(emu, 0x0111), calls(emu, 0x0113)
    for remaining in range(5, -1, -1):
        result = context.capture()
        assert context.remaining == remaining
        assert (result is not None) == (remaining == 0)
        assert calls(emu, 0x0113) - before[1] == (1 if remaining == 0 else 0)
    assert calls(emu, 0x0111) - before[0] == 6
    return context.template_id


@pytest.mark.parametrize("entry", ["hid", "ccid"])
@pytest.mark.parametrize("protocol", [PinProtocolV1, PinProtocolV2])
def test_standard_six_sample_management_and_three_authorization_paths(wire_emulator, entry, protocol):
    emu = wire_emulator
    pin = set_pin(emu)
    rp = RelyingParty()
    rp.register(emu, pin)
    card = Card(emu.ccid_port) if entry == "ccid" else None
    try:
        ctap = Ctap2(card) if card else emu.ctap
        api = bio(emu, pin, ctap, protocol())
        assert api.get_fingerprint_sensor_info()[3] == 6
        template = enroll(emu, api)
        api.set_name(template, "测试指纹")
        assert api.enumerate_enrollments() == {template: "测试指纹"}
        for mode in ["direct", "uv-token", "pin-token"]:
            before = calls(emu, 0x0121), emu.prompts
            with emu.confirmations(Reply.APPROVE if mode == "pin-token" else Reply.WAIT):
                result, digest = assertion(emu, ctap, rp, mode, pin)
            assert_signed(rp, result, digest)
            assert calls(emu, 0x0121) - before[0] == (0 if mode == "pin-token" else 1)
            assert emu.prompts - before[1] == (1 if mode == "pin-token" else 0)
        bio(emu, pin, ctap, protocol()).remove_enrollment(template)
        assert ctap.get_info().options["uv"] is False
    finally:
        if card:
            card.close()
    emu.restart()
    rp.authenticate(emu, pin)
    assert emu.store.stat().st_size == 1_048_592


@pytest.mark.parametrize("mode,feedback", [("poor", 7), ("nofinger", 13)])
def test_invalid_samples_leave_the_real_count_unchanged(wire_emulator, mode, feedback):
    emu = wire_emulator
    api = bio(emu, set_pin(emu))
    context = api.enroll()
    emu.script.write_text(mode + "\n")
    with pytest.raises(CaptureError) as caught:
        context.capture()
    assert caught.value.code == feedback and context.remaining == 6
    assert calls(emu, 0x0113) == 0
    emu.script.write_text("good\n")
    for remaining in range(5, -1, -1):
        context.capture()
        assert context.remaining == remaining
    assert calls(emu, 0x0111) == 7
    assert calls(emu, 0x0113) == 1


def test_up_only_without_templates_never_sends_capture_or_match(wire_emulator):
    emu = wire_emulator
    pin = set_pin(emu)
    rp = RelyingParty()
    rp.register(emu, pin)
    before = calls(emu, 0x0121), calls(emu, 0x0111)
    digest = secrets.token_bytes(32)
    with emu.confirmations():
        answer = emu.ctap.get_assertion(rp.id, digest,
            [{"type": "public-key", "id": rp.credential.credential_id}])
    assert answer.auth_data.is_user_present() and not answer.auth_data.is_user_verified()
    rp.credential.public_key.verify(bytes(answer.auth_data) + digest, answer.signature)
    assert (calls(emu, 0x0121), calls(emu, 0x0111)) == before


@pytest.mark.parametrize("mode,code", [("nomatch", CtapError.ERR.UV_INVALID),
    ("nofinger", CtapError.ERR.USER_ACTION_TIMEOUT),
    ("unknown", CtapError.ERR.UV_INVALID), ("corrupt", CtapError.ERR.OTHER),
    ("duplicate", CtapError.ERR.OTHER)])
def test_bad_wire_results_cannot_create_uv_and_pin_still_verifies(wire_emulator, mode, code):
    emu = wire_emulator
    pin = set_pin(emu)
    rp = RelyingParty()
    rp.register(emu, pin)
    enroll(emu, bio(emu, pin))
    emu.script.write_text(mode + "\n")
    with emu.confirmations(Reply.WAIT):
        refused(code, lambda: ClientPin(emu.ctap).get_uv_token(PERM.GET_ASSERTION, rp.id))
    if mode == "nofinger":
        assert ClientPin(emu.ctap).get_uv_retries() == 8
    before = calls(emu, 0x0121)
    with emu.confirmations():
        answer, digest = assertion(emu, emu.ctap, rp, "pin-token", pin)
    assert_signed(rp, answer, digest)
    assert calls(emu, 0x0121) == before


def test_unknown_save_is_not_repeated_and_restarts_as_unapproved(wire_emulator):
    emu = wire_emulator
    pin = set_pin(emu)
    rp = RelyingParty()
    rp.register(emu, pin)
    api = bio(emu, pin)
    context = api.enroll()
    for _ in range(5):
        context.capture()
    template = context.template_id
    emu.script.write_text("save-lost\n")
    refused(CtapError.ERR.OTHER, context.capture)
    assert calls(emu, 0x0113) == 1
    assert emu.ctap.get_info().options["uv"] is False
    emu.script.write_text("good\n")
    emu.restart()
    api = bio(emu, pin)
    assert api.enumerate_enrollments() == {template: ""}
    assert emu.ctap.get_info().options["uv"] is False
    api.remove_enrollment(template)
    assert api.enumerate_enrollments() == {}
    rp.authenticate(emu, pin)


def test_cancel_after_real_start_never_saves_and_can_finish_draining(wire_emulator):
    emu = wire_emulator
    api = bio(emu, set_pin(emu))
    emu.script.write_text("wait\n")
    event = threading.Event()
    errors = []
    def cancel_after_start():
        try:
            emu.wait_for("fpm-wire: command 0111")
            event.set()
        except Exception as error:
            errors.append(error)
    watcher = threading.Thread(target=cancel_after_start)
    watcher.start()
    try:
        refused(CtapError.ERR.KEEPALIVE_CANCEL, lambda: api.enroll_begin(event=event))
    finally:
        watcher.join(timeout=5)
    assert not watcher.is_alive() and not errors
    emu.wait_for("fpm-wire: command 0115")
    deadline = time.monotonic() + 2
    while calls(emu, 0x0112) < 1 and time.monotonic() < deadline:
        time.sleep(0.01)
    assert calls(emu, 0x0112) >= 1
    emu.script.write_text("good\n")
    # Allow the old terminal frame to finish its deliberately fragmented transport.
    time.sleep(0.1)
    enroll(emu, api)
    assert calls(emu, 0x0113) == 1


def test_full_webauthn_uv_has_no_extra_button_and_uses_registered_key(wire_emulator):
    emu = wire_emulator
    pin = set_pin(emu)
    enroll(emu, bio(emu, pin))
    rp = RelyingParty()
    from ceremony import RK, UV
    options, state = rp.server.register_begin(rp.user, resident_key_requirement=RK.REQUIRED,
        user_verification=UV.REQUIRED)
    before = calls(emu, 0x0121), emu.prompts
    client = rp.client(emu, Finger())
    with emu.confirmations(Reply.WAIT):
        response = client.make_credential(options.public_key)
    rp.credential = rp.server.register_complete(state, response).credential_data
    options, state = rp.request()
    client = rp.client(emu, Finger())
    with emu.confirmations(Reply.WAIT):
        response = client.get_assertion(options.public_key).get_response(0)
    assert rp.server.authenticate_complete(state, [rp.credential], response) == rp.credential
    assert calls(emu, 0x0121) - before[0] == 2
    assert emu.prompts == before[1]


def test_unknown_delete_remains_disabled_and_explicit_cleanup_preserves_pin(wire_emulator):
    emu = wire_emulator
    pin = set_pin(emu)
    rp = RelyingParty()
    rp.register(emu, pin)
    api = bio(emu, pin)
    template = enroll(emu, api)
    emu.script.write_text("delete-lost\n")
    refused(CtapError.ERR.OTHER, lambda: api.remove_enrollment(template))
    assert calls(emu, 0x0131) == 1
    assert emu.ctap.get_info().options["uv"] is False
    emu.script.write_text("good\n")
    emu.restart()
    api = bio(emu, pin)
    assert api.enumerate_enrollments() == {template: ""}
    api.remove_enrollment(template)
    assert api.enumerate_enrollments() == {}
    rp.authenticate(emu, pin)


@pytest.mark.parametrize("lost", [False, True])
def test_reset_clears_the_whole_library_and_only_then_invalidates_old_credentials(wire_emulator, lost):
    emu = wire_emulator
    pin = set_pin(emu)
    rp = RelyingParty()
    rp.register(emu, pin)
    enroll(emu, bio(emu, pin))
    stock = bytearray(emu.inventory.read_bytes())
    stock[99 // 8] |= 1 << (99 % 8)
    emu.inventory.write_bytes(stock)
    emu.restart()
    assert calls(emu, 0x0131) == 0, "startup must not clear unknown inventory"
    if lost:
        emu.script.write_text("delete-lost\n")
        with emu.confirmations():
            refused(CtapError.ERR.OTHER, emu.ctap.reset)
        assert emu.ctap.get_info().options["clientPin"] is True
        assert calls(emu, 0x0131) == 1
        emu.script.write_text("good\n")
        emu.restart()
        assert calls(emu, 0x0131) == 0
    with emu.confirmations():
        emu.ctap.reset()
    assert not any(emu.inventory.read_bytes())
    info = emu.ctap.get_info()
    assert info.options["clientPin"] is False and info.options["uv"] is False
    refused(CtapError.ERR.NO_CREDENTIALS, lambda: emu.ctap.get_assertion(rp.id,
        secrets.token_bytes(32), [{"type": "public-key", "id": rp.credential.credential_id}],
        options={"up": False}))
