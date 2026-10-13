# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Power facts come from the byte module, below actual CTAP and RP verification."""

import secrets
import time

import pytest
from fido2.ctap import CtapError
from fido2.ctap2.pin import ClientPin
from fido2.ctap2.config import Config
from fido2.ctap2.credman import CredentialManagement

from ceremony import RelyingParty, set_pin
from session import Reply
from test_protocol import bio, calls, enroll, refused, PERM
from test_authorization import assertion, assert_signed

EMPTY_LARGE_BLOB_LENGTH = 17


def power(emu, state):
    return emu.lines.count(f"fpm-power: {state}")


def idle(emu):
    if not emu.controlled:
        return
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        if power(emu, "on") == power(emu, "off"):
            return
        time.sleep(0.01)
    pytest.fail("a released, drained module did not turn off")


def test_idle_up_pin_and_bad_authentication_never_start_the_supply(wire_emulator):
    emu = wire_emulator
    pin = set_pin(emu)
    rp = RelyingParty()
    rp.register(emu, pin)
    for _ in range(2):
        assert emu.ctap.get_info().options["uv"] is False
        digest = secrets.token_bytes(32)
        with emu.confirmations():
            response = emu.ctap.get_assertion(rp.id, digest,
                [{"type": "public-key", "id": rp.credential.credential_id}])
        rp.credential.public_key.verify(bytes(response.auth_data) + digest, response.signature)
        assert response.auth_data.is_user_present() and not response.auth_data.is_user_verified()
    refused(CtapError.ERR.PUAT_REQUIRED, lambda: emu.ctap.bio_enrollment(1, 1))
    assert power(emu, "on") == 0
    assert calls(emu, 0x0121) == 0
    if emu.controlled:
        assert calls(emu, 0x0203) == 0, "pure host processing must not even open UART"


def test_six_sample_gaps_and_post_save_inventory_hold_the_supply(wire_emulator):
    emu = wire_emulator
    pin = set_pin(emu)
    api = bio(emu, pin)
    context = api.enroll()
    for remaining in range(5, 0, -1):
        context.capture()
        assert context.remaining == remaining
        time.sleep(0.05)
        assert power(emu, "off") == 0
        assert calls(emu, 0x020c) == 0, "temporary samples must survive between requests"
    context.capture()
    assert calls(emu, 0x0113) == 1
    assert calls(emu, 0x0133) >= 700, "post-save inventory must run before release"
    idle(emu)
    if emu.controlled:
        assert (power(emu, "on"), power(emu, "off")) == (1, 1)
    assert api.enumerate_enrollments() == {context.template_id: ""}


def test_bad_fingerprint_token_parameters_are_refused_before_any_inventory_or_power(wire_emulator):
    emu = wire_emulator
    pin = set_pin(emu)
    rp = RelyingParty()
    rp.register(emu, pin)
    enroll(emu, bio(emu, pin))
    cp = ClientPin(emu.ctap)
    with emu.confirmations(Reply.WAIT):
        token = cp.get_uv_token(PERM.GET_ASSERTION, rp.id)
    idle(emu)
    before = power(emu, "on"), calls(emu, 0x0203)
    digest = secrets.token_bytes(32)
    auth = bytearray(cp.protocol.authenticate(token, digest))
    auth[0] ^= 1
    kwargs = {"pin_uv_protocol": cp.protocol.VERSION, "pin_uv_param": bytes(auth)}
    requests = [
        lambda: emu.ctap.get_assertion(rp.id, digest,
            [{"type": "public-key", "id": rp.credential.credential_id}], **kwargs),
        lambda: emu.ctap.make_credential(digest, {"id": rp.id},
            {"id": b"synthetic"}, [{"type": "public-key", "alg": -7}], **kwargs),
        lambda: emu.ctap.config(Config.CMD.TOGGLE_ALWAYS_UV, **kwargs),
        lambda: emu.ctap.credential_mgmt(CredentialManagement.CMD.GET_CREDS_METADATA, **kwargs),
    ]
    if emu.ctap.info.options.get("largeBlobs"):
        requests.append(lambda: emu.ctap.large_blobs(0, set=b"\x80", length=EMPTY_LARGE_BLOB_LENGTH, **kwargs))
    for request in requests:
        refused(CtapError.ERR.PIN_AUTH_INVALID, request)
        assert (power(emu, "on"), calls(emu, 0x0203)) == before
    with emu.confirmations(Reply.WAIT):
        result, digest = assertion(emu, emu.ctap, rp, "uv-token", pin)
    assert_signed(rp, result, digest)


@pytest.mark.parametrize("ending", ["cancel", "new-command"])
def test_session_end_releases_power_and_old_capture_cannot_resume(wire_emulator, ending):
    emu = wire_emulator
    pin = set_pin(emu)
    api = bio(emu, pin)
    context = api.enroll()
    context.capture()
    if ending == "cancel":
        api.enroll_cancel()
    else:
        assert emu.ctap.get_info().options["uv"] is False
        assert power(emu, "off") == 0, "GetInfo must permit a second client's continued enrollment"
        with emu.confirmations():
            emu.ctap.selection()
    idle(emu)
    refused(CtapError.ERR.NOT_ALLOWED, context.capture)
    idle(emu)
    assert emu.ctap.get_info().options["uv"] is False
    enroll(emu, bio(emu, pin))
    idle(emu)
    assert emu.ctap.get_info().options["uv"] is True


@pytest.mark.parametrize("fault", ["sleep-lost", "boot-lost"])
def test_start_or_stop_fault_keeps_configuration_and_pin_fallback(wire_emulator, fault):
    emu = wire_emulator
    if not emu.controlled:
        pytest.skip("fault targets the switched supply profile")
    pin = set_pin(emu)
    rp = RelyingParty()
    rp.register(emu, pin)
    enroll(emu, bio(emu, pin))
    idle(emu)
    emu.script.write_text(fault + "\n")
    before = power(emu, "on"), power(emu, "off")
    with emu.confirmations(Reply.WAIT):
        if fault == "boot-lost":
            refused(CtapError.ERR.OTHER,
                lambda: ClientPin(emu.ctap).get_uv_token(PERM.GET_ASSERTION, rp.id))
            assert ClientPin(emu.ctap).get_uv_retries() == 8
        else:
            result, digest = assertion(emu, emu.ctap, rp, "direct", pin)
            assert_signed(rp, result, digest)
    time.sleep(3.2)
    assert power(emu, "on") == before[0] + 1
    assert power(emu, "off") == before[1], "unconfirmed shutdown must not cut or restart power"
    assert emu.ctap.get_info().options["uv"] is True
    with emu.confirmations():
        result, digest = assertion(emu, emu.ctap, rp, "pin-token", pin)
    assert_signed(rp, result, digest)
    assert power(emu, "on") == before[0] + 1
