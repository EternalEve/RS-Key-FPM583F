# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Pinned standard management API through the shared production FIDO policy."""

import hashlib
import secrets
import threading

import pytest
from fido2.ctap import CtapError
from fido2.ctap2 import Ctap2, FPBioEnrollment, CaptureError
from fido2.ctap2.pin import ClientPin, PinProtocolV1, PinProtocolV2
from fido2.hid import CTAPHID, STATUS

from ccid import Card
from ceremony import RelyingParty, set_pin
from session import Reply
from test_authorization import Finger, assert_signed

PERM = ClientPin.PERMISSION
CMD = FPBioEnrollment.CMD
PARAM = FPBioEnrollment.PARAM


def refused(code, call):
    with pytest.raises(CtapError) as caught:
        call()
    assert caught.value.code == code


def bio(emu, pin, ctap=None, protocol=None, permissions=PERM.BIO_ENROLL):
    ctap = ctap or emu.ctap
    cp = ClientPin(ctap, protocol)
    with emu.confirmations():
        token = cp.get_pin_token(pin, permissions)
    return FPBioEnrollment(ctap, cp.protocol, token)


def enrolled(emu, api):
    context = api.enroll()
    with emu.confirmations(Reply.WAIT):
        assert context.capture() is None
        assert context.remaining == 2
        assert context.capture() is None
        assert context.remaining == 1
        template = context.capture()
    assert len(template) == 16
    assert context.remaining == 0
    return template


@pytest.mark.parametrize("entry", ["hid", "ccid"])
@pytest.mark.parametrize("protocol", [PinProtocolV1, PinProtocolV2])
def test_management_and_original_public_key_survive_last_delete(emulator, entry, protocol):
    pin = set_pin(emulator)
    rp = RelyingParty()
    rp.register(emulator, pin)
    card = Card(emulator.ccid_port) if entry == "ccid" else None
    try:
        ctap = Ctap2(card) if card else emulator.ctap
        api = bio(emulator, pin, ctap, protocol())
        template = enrolled(emulator, api)
        assert api.enumerate_enrollments() == {template: ""}
        name = "指纹" + "🖐" * 14 + "ab"
        assert len(name.encode()) == 64
        api.set_name(template, name)
        refused(CtapError.ERR.INVALID_LENGTH, lambda: api.set_name(template, name + "x"))
        assert api.enumerate_enrollments() == {template: name}
        info = ctap.get_info()
        assert info.options["bioEnroll"] is True and info.options["uv"] is True
        assert info.options["uvBioEnroll"] is False
        assert info.uv_modality == 2
        digest = secrets.token_bytes(32)
        with emulator.confirmations(Reply.WAIT):
            result = ctap.get_assertion(rp.id, digest, [{"type": "public-key", "id": rp.credential.credential_id}], options={"uv": True})
        assert_signed(rp, result, digest)
        refused(CtapError.ERR.PIN_AUTH_INVALID, lambda: api.remove_enrollment(template))
        api = bio(emulator, pin, ctap, protocol())
        api.remove_enrollment(template)
        assert api.enumerate_enrollments() == {}
        assert ctap.get_info().options["bioEnroll"] is False
        assert ctap.get_info().options["clientPin"] is True
    finally:
        if card:
            card.close()
    emulator.restart()
    emulator.ctap = Ctap2(emulator.device)
    rp.authenticate(emulator, pin)


def test_anonymous_queries_cancel_and_permission_failure_have_no_writes(emulator):
    api = FPBioEnrollment(emulator.ctap, None, None)
    info = api.get_fingerprint_sensor_info()
    assert info[3] == 3 and info[8] == 64
    api.enroll_cancel()
    assert emulator.ctap.info.options["bioEnroll"] is False
    before = emulator.inventory.read_bytes()
    refused(CtapError.ERR.PUAT_REQUIRED, lambda: emulator.ctap.bio_enrollment(1, CMD.ENROLL_BEGIN))
    pin = set_pin(emulator)
    wrong = bio(emulator, pin, permissions=PERM.GET_ASSERTION)
    refused(CtapError.ERR.PIN_AUTH_INVALID, wrong.enroll_begin)
    assert emulator.inventory.read_bytes() == before
    assert not any("fingerprint: management" in line for line in emulator.lines)


@pytest.mark.parametrize("event,feedback", [("poor-quality", 7), ("no-transition", 14), ("no-activity", 13)])
def test_quality_feedback_preserves_standard_capture_context(emulator, event, feedback):
    pin = set_pin(emulator)
    api = bio(emulator, pin)
    context = api.enroll()
    emulator.script.write_text(event + " 0 0 0\n")
    with pytest.raises(CaptureError) as caught:
        context.capture()
    assert caught.value.code == feedback and context.remaining == 3
    template = context.template_id
    emulator.script.write_text("match 0 0 0\n")
    assert context.capture() is None
    assert context.capture() is None
    assert context.capture() == template
    assert api.enumerate_enrollments() == {template: ""}


def test_saved_unknown_result_is_isolated_then_standard_cleanup_succeeds(emulator):
    pin = set_pin(emulator)
    api = bio(emulator, pin)
    context = api.enroll()
    assert context.capture() is None
    assert context.capture() is None
    template = context.template_id
    emulator.script.write_text("save-unknown 0 0 0\n")
    refused(CtapError.ERR.OTHER, context.capture)
    assert emulator.ctap.get_info().options["uv"] is False
    emulator.restart()
    api = bio(emulator, pin)
    assert api.enumerate_enrollments() == {template: ""}
    assert emulator.ctap.get_info().options["uv"] is False
    emulator.script.write_text("match 0 0 0\n")
    api.remove_enrollment(template)
    assert api.enumerate_enrollments() == {}


def test_failed_delete_stays_disabled_after_restart_and_slot_reuse(emulator):
    pin = set_pin(emulator)
    api = bio(emulator, pin)
    template = enrolled(emulator, api)
    emulator.script.write_text("delete-failure 0 0 0\n")
    refused(CtapError.ERR.OTHER, lambda: api.remove_enrollment(template))
    assert emulator.ctap.get_info().options["uv"] is False
    emulator.restart()
    api = bio(emulator, pin)
    assert api.enumerate_enrollments() == {template: ""}
    emulator.script.write_text("match 0 0 0\n")
    api.remove_enrollment(template)
    replacement = enrolled(emulator, api)
    assert replacement != template
    refused(CtapError.ERR.INVALID_OPTION, lambda: api.remove_enrollment(template))
    assert api.enumerate_enrollments() == {replacement: ""}


def test_cancel_between_requests_new_begin_and_old_id(emulator):
    pin = set_pin(emulator)
    api = bio(emulator, pin)
    old, _, _ = api.enroll_begin()
    anonymous = FPBioEnrollment(emulator.ctap, None, None)
    anonymous.enroll_cancel()
    refused(CtapError.ERR.NOT_ALLOWED, lambda: api.enroll_capture_next(old))
    first, _, _ = api.enroll_begin()
    second, _, _ = api.enroll_begin()
    assert first != second
    refused(CtapError.ERR.NOT_ALLOWED, lambda: api.enroll_capture_next(first))
    anonymous.enroll_cancel()
    anonymous.enroll_cancel()
    assert api.enumerate_enrollments() == {}


def test_hid_cancel_during_sample_keeps_uncommitted_template_unauthorized(emulator):
    pin = set_pin(emulator)
    api = bio(emulator, pin)
    event = threading.Event()
    emulator.script.write_text("wait 0 0 0\n")
    with emulator.confirmations(Reply.WAIT):
        refused(CtapError.ERR.KEEPALIVE_CANCEL, lambda: api.enroll_begin(event=event, on_keepalive=lambda status: event.set() if status == STATUS.UPNEEDED else None))
    assert emulator.ctap.get_info().options["uv"] is False


def test_complete_names_and_ids_survive_restart_at_capacity(emulator):
    pin = set_pin(emulator)
    api = bio(emulator, pin)
    templates = {}
    for _ in range(16):
        template = enrolled(emulator, api)
        name = "🖐" * 16
        api.set_name(template, name)
        templates[template] = name
    before = emulator.inventory.read_bytes()
    refused(CtapError.ERR.FP_DATABASE_FULL, api.enroll_begin)
    assert emulator.inventory.read_bytes() == before
    assert api.enumerate_enrollments() == templates
    with Card(emulator.ccid_port) as card:
        cross = bio(emulator, pin, Ctap2(card))
        assert cross.enumerate_enrollments() == templates
    assert emulator.store.stat().st_size == 1048592, "target 896 KiB main + 128 KiB counter, plus header"
    emulator.restart()
    assert bio(emulator, pin).enumerate_enrollments() == templates


def test_module_clear_failure_keeps_reset_marker_then_authorized_reset_recovers(emulator):
    pin = set_pin(emulator)
    api = bio(emulator, pin)
    enrolled(emulator, api)
    emulator.script.write_text("clear-failure 0 0 0\n")
    with emulator.confirmations():
        refused(CtapError.ERR.OTHER, emulator.ctap.reset)
    emulator.restart()
    assert emulator.ctap.info.options["uv"] is False
    emulator.script.write_text("match 0 0 0\n")
    with emulator.confirmations():
        emulator.ctap.reset()
    assert not any(emulator.inventory.read_bytes())
    assert emulator.ctap.get_info().options["clientPin"] is False


@pytest.mark.parametrize("entry", ["hid", "ccid"])
def test_standard_webauthn_fingerprint_registration_and_login_at_rp(emulator, entry):
    from ceremony import RK, UV
    pin = set_pin(emulator)
    api = bio(emulator, pin)
    enrolled(emulator, api)
    card = Card(emulator.ccid_port) if entry == "ccid" else None
    original = emulator.device, emulator.ctap
    try:
        if card:
            emulator.device, emulator.ctap = card, Ctap2(card)
        rp = RelyingParty()
        options, state = rp.server.register_begin(rp.user, resident_key_requirement=RK.REQUIRED, user_verification=UV.REQUIRED)
        finger = Finger()
        client = rp.client(emulator, finger)
        before = emulator.prompts
        with emulator.confirmations(Reply.WAIT):
            response = client.make_credential(options.public_key)
        data = rp.server.register_complete(state, response)
        assert data.is_user_present() and data.is_user_verified()
        rp.credential = data.credential_data
        options, state = rp.request()
        with emulator.confirmations(Reply.WAIT):
            selection = client.get_assertion(options.public_key)
        verified = rp.server.authenticate_complete(state, [rp.credential], selection.get_response(0))
        assert verified == rp.credential
        assert emulator.prompts == before
    finally:
        emulator.device, emulator.ctap = original
        if card:
            card.close()


def test_cross_transport_capture_and_anonymous_standard_cancel_share_one_session(emulator):
    pin = set_pin(emulator)
    api = bio(emulator, pin)
    template, _, remaining = api.enroll_begin()
    assert remaining == 2
    with Card(emulator.ccid_port) as card:
        other = FPBioEnrollment(Ctap2(card), api.pin_uv_protocol, api.pin_uv_token)
        assert other.enroll_capture_next(template)[1] == 1
        FPBioEnrollment(Ctap2(card), None, None).enroll_cancel()
    refused(CtapError.ERR.NOT_ALLOWED, lambda: api.enroll_capture_next(template))
    assert api.enumerate_enrollments() == {}


def test_tampered_authenticated_parameters_and_unsupported_protocol_do_not_write(emulator):
    from fido2 import cbor
    pin = set_pin(emulator)
    api = bio(emulator, pin)
    template = enrolled(emulator, api)
    params = {PARAM.TEMPLATE_ID: template, PARAM.TEMPLATE_NAME: "original"}
    auth = api.pin_uv_protocol.authenticate(api.pin_uv_token, bytes([1, CMD.SET_NAME]) + cbor.encode(params))
    params[PARAM.TEMPLATE_NAME] = "changed"
    before = emulator.inventory.read_bytes()
    refused(CtapError.ERR.PIN_AUTH_INVALID, lambda: emulator.ctap.bio_enrollment(1, CMD.SET_NAME, params, 2, auth))
    refused(CtapError.ERR.INVALID_PARAMETER, lambda: emulator.ctap.bio_enrollment(1, CMD.SET_NAME, params, 3, auth))
    assert api.enumerate_enrollments() == {template: ""}
    assert emulator.inventory.read_bytes() == before


def test_up_only_and_pin_tokens_do_not_invoke_matching_during_module_drain(emulator):
    pin = set_pin(emulator)
    rp = RelyingParty()
    rp.register(emulator, pin)
    api = bio(emulator, pin)
    enrolled(emulator, api)
    api = bio(emulator, pin)
    emulator.script.write_text("wait 0 0 1500\n")
    event = threading.Event()
    with emulator.confirmations(Reply.WAIT):
        refused(CtapError.ERR.KEEPALIVE_CANCEL, lambda: api.enroll_begin(event=event, on_keepalive=lambda status: event.set() if status == STATUS.UPNEEDED else None))
    matches = sum("fingerprint: matching " in line for line in emulator.lines)
    cp = ClientPin(emulator.ctap)
    with emulator.confirmations():
        token = cp.get_pin_token(pin, PERM.GET_ASSERTION, rp.id)
        digest = secrets.token_bytes(32)
        result = emulator.ctap.get_assertion(rp.id, digest, [{"type": "public-key", "id": rp.credential.credential_id}], pin_uv_param=cp.protocol.authenticate(token, digest), pin_uv_protocol=cp.protocol.VERSION)
    assert_signed(rp, result, digest)
    with emulator.confirmations():
        digest = secrets.token_bytes(32)
        result = emulator.ctap.get_assertion(rp.id, digest, [{"type": "public-key", "id": rp.credential.credential_id}], options={"uv": False})
    assert result.auth_data.is_user_present() and not result.auth_data.is_user_verified()
    rp.credential.public_key.verify(bytes(result.auth_data) + digest, result.signature)
    assert sum("fingerprint: matching " in line for line in emulator.lines) == matches


def test_restarted_partial_enrollment_reclaims_its_own_reserved_slot_with_pin(emulator):
    pin = set_pin(emulator)
    api = bio(emulator, pin)
    old, _, _ = api.enroll_begin()
    assert not any(emulator.inventory.read_bytes())
    emulator.restart()
    api = bio(emulator, pin)
    template = enrolled(emulator, api)
    assert template != old
    assert api.enumerate_enrollments() == {template: ""}


def test_disconnect_does_not_let_a_new_connection_resume_an_old_enrollment(emulator):
    from transport import connect
    pin = set_pin(emulator)
    api = bio(emulator, pin)
    template, _, _ = api.enroll_begin()
    emulator.device.close()
    emulator.wait_for("fingerprint: connection ended channel")
    device, connection = connect(emulator.port)
    emulator.connection = connection
    emulator.device = type(emulator.device)(device, connection)
    emulator.ctap = Ctap2(emulator.device)
    other = FPBioEnrollment(emulator.ctap, api.pin_uv_protocol, api.pin_uv_token)
    refused(CtapError.ERR.NOT_ALLOWED, lambda: other.enroll_capture_next(template))
    replacement = enrolled(emulator, bio(emulator, pin))
    assert replacement != template
