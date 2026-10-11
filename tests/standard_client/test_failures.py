# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Meaningful refusal, observed waiting/cancellation, and transport failure causes."""

import json
import secrets
import threading
from dataclasses import replace

import pytest
from fido2.client import ClientError, PinRequiredError
from fido2.ctap import CtapError
from fido2.ctap2.pin import ClientPin

from ceremony import Interaction, RK, UV, RelyingParty, set_pin
from session import STOP_TIMEOUT, Emulator, Reply


def up_only_credential(emu):
    rp = RelyingParty()
    rp.register(emu, None, resident=RK.DISCOURAGED, uv=UV.DISCOURAGED)
    return rp


def assert_ctap_cause(error, expected):
    assert isinstance(error.cause, CtapError), "failure must come from the authenticator"
    assert error.cause.code == expected
    print("refusal " + json.dumps({"client_error": error.code.name,
                                   "ctap_error": error.cause.code.name}))


def test_explicit_denial_cannot_authenticate(emulator):
    rp = up_only_credential(emulator)
    before = emulator.prompts
    with pytest.raises(ClientError) as caught:
        rp.authenticate(emulator, None, discoverable=False,
                        uv=UV.DISCOURAGED, reply=Reply.DENY)
    assert_ctap_cause(caught.value, CtapError.ERR.OPERATION_DENIED)
    assert emulator.prompts == before + 1


def test_keepalive_cancel_then_new_operation_on_same_connection(emulator):
    rp = up_only_credential(emulator)
    interaction = Interaction(None)
    interaction.cancel = threading.Event()
    client = rp.client(emulator, interaction)
    options, _ = rp.request(discoverable=False, uv=UV.DISCOURAGED)
    before = emulator.prompts
    with pytest.raises(ClientError) as caught:
        with emulator.confirmations(Reply.WAIT):
            client.get_assertion(options.public_key, event=interaction.cancel)
    assert_ctap_cause(caught.value, CtapError.ERR.KEEPALIVE_CANCEL)
    assert interaction.prompt_count >= 1, "library must observe UPNEEDED before cancelling"
    assert emulator.prompts == before + 1
    emulator.wait_for("cancelled by the host")
    response, _, _ = rp.authenticate(emulator, None, discoverable=False, uv=UV.DISCOURAGED)
    assert response.response.authenticator_data.is_user_present()
    assert emulator.prompts == before + 2, "next operation needs a fresh confirmation"
    print("cancel " + json.dumps({"upneeded_callbacks": interaction.prompt_count,
                                 "next_operation_verified": True, "same_connection": True}))


def test_unanswered_presence_hits_transport_deadline(emulator):
    rp = up_only_credential(emulator)
    interaction = Interaction(None)
    client = rp.client(emulator, interaction)
    options, _ = rp.request(discoverable=False, uv=UV.DISCOURAGED)
    with pytest.raises(TimeoutError):
        with emulator.confirmations(Reply.WAIT, timeout=0.5):
            client.get_assertion(options.public_key)
    assert interaction.prompt_count >= 1, "keepalive is progress, never success"
    assert emulator.connection.sock.fileno() == -1
    print("transport timeout; connection invalidated while confirmation remained unanswered")


def test_closed_connection_at_observed_wait_preserves_transport_error(emulator):
    rp = up_only_credential(emulator)

    class Disconnect(Interaction):
        def prompt_up(self):
            super().prompt_up()
            emulator.connection.close()

    interaction = Disconnect(None)
    client = rp.client(emulator, interaction)
    options, _ = rp.request(discoverable=False, uv=UV.DISCOURAGED)
    with pytest.raises(OSError):
        with emulator.confirmations(Reply.WAIT):
            client.get_assertion(options.public_key)
    assert interaction.prompt_count >= 1
    assert emulator.connection.sock.fileno() == -1


@pytest.mark.parametrize("provide_wrong_pin", [True, False])
def test_required_uv_rejects_one_wrong_pin_or_missing_pin(emulator, provide_wrong_pin):
    pin = set_pin(emulator)
    rp = RelyingParty()
    rp.register(emulator, pin)
    with emulator.connection.operation():
        retries_before = ClientPin(emulator.ctap).get_pin_retries()[0]
    interaction = Interaction(secrets.token_hex(6) if provide_wrong_pin else None)
    client = rp.client(emulator, interaction)
    options, _ = rp.request()
    with pytest.raises(ClientError) as caught:
        with emulator.confirmations():
            client.get_assertion(options.public_key)
    assert interaction.pin_requests == [(2, rp.id)], "only one PIN attempt is allowed"
    with emulator.connection.operation():
        retries_after = ClientPin(emulator.ctap).get_pin_retries()[0]
    if provide_wrong_pin:
        assert_ctap_cause(caught.value, CtapError.ERR.PIN_INVALID)
        assert retries_after == retries_before - 1
    else:
        assert isinstance(caught.value, PinRequiredError)
        assert retries_after == retries_before


def test_credential_from_another_rp_is_refused_by_authenticator(emulator):
    original = up_only_credential(emulator)
    other = RelyingParty()
    other.credential = original.credential
    with pytest.raises(ClientError) as caught:
        other.authenticate(emulator, None, discoverable=False, uv=UV.DISCOURAGED)
    assert_ctap_cause(caught.value, CtapError.ERR.NO_CREDENTIALS)


def test_client_keeps_default_origin_rp_validation(emulator):
    rp = up_only_credential(emulator)
    client = rp.client(emulator, Interaction(None))
    options, _ = rp.request(discoverable=False, uv=UV.DISCOURAGED)
    invalid = replace(options.public_key, rp_id="unrelated.example.org")
    before = emulator.prompts
    with pytest.raises(ClientError) as caught:
        with emulator.confirmations():
            client.get_assertion(invalid)
    assert caught.value.code == ClientError.ERR.BAD_REQUEST
    assert emulator.prompts == before, "invalid RP must be rejected before any ceremony"


def test_incompatible_store_is_refused_without_reinitializing(binary, tmp_path):
    store = tmp_path / "incompatible.store"
    original = b"incompatible synthetic flash layout"
    store.write_bytes(original)
    emu = Emulator(binary, store)
    try:
        with pytest.raises(AssertionError, match="not an rsk-emu flash image of this geometry"):
            emu.start()
        assert store.read_bytes() == original, "a bad image must not be silently erased"
        assert emu.process.wait(timeout=STOP_TIMEOUT) == 0
        emu.stop(expected_returncode=0)
    finally:
        emu.stop()
