# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Passkey/PIN, UP-only, RP rejection and normal restart through python-fido2."""

import json
from dataclasses import replace

import pytest

from ceremony import RK, UV, RelyingParty, set_pin


def test_handshake_getinfo_and_fresh_store(emulator):
    with emulator.connection.operation():
        assert emulator.device.ping(b"RS-Key standard client") == b"RS-Key standard client"
        info = emulator.ctap.get_info()
    assert "FIDO_2_1" in info.versions
    assert info.pin_uv_protocols == [2, 1]
    assert info.options["clientPin"] is False
    assert info.options["rk"] is True
    assert info.options.get("uv") is not True
    assert info.options.get("alwaysUv") is not True
    assert "bioEnroll" not in info.options
    print("getInfo " + json.dumps({
        "versions": info.versions, "pin_uv_protocols": info.pin_uv_protocols,
        "algorithms": info.algorithms, "options": info.options,
        "max_msg_size": info.max_msg_size,
    }))


def test_passkey_pin_uv_rp_verification_and_restart(emulator):
    pin = set_pin(emulator)
    rp = RelyingParty()
    _, registered, registration_interaction = rp.register(emulator, pin)
    response, state, interaction = rp.authenticate(emulator, pin)
    assert registration_interaction.pin_requests == [(1, rp.id)]
    assert interaction.pin_requests == [(2, rp.id)]
    assert emulator.prompts >= 4, "permission consent and ceremonies must be explicitly answered"
    counter = response.response.authenticator_data.counter
    print("passkey " + json.dumps({
        "algorithm": registered.credential_data.public_key[3],
        "registration_uv": registered.is_user_verified(),
        "assertion_uv": response.response.authenticator_data.is_user_verified(),
        "counter_before_restart": counter, "terminal_prompts": emulator.prompts,
        "pin_permissions": interaction.pin_requests,
    }))

    # These copies reach distinct RP checks; an unrelated earlier failure cannot count as a kill.
    _, new_state = rp.request()
    with pytest.raises(ValueError, match="^Wrong challenge in response\\.$"):
        rp.server.authenticate_complete(new_state, [rp.credential], response)
    damaged = replace(response, response=replace(
        response.response,
        signature=response.response.signature[:-1] + bytes([response.response.signature[-1] ^ 1]),
    ))
    with pytest.raises(ValueError, match="^Invalid signature\\.$"):
        rp.server.authenticate_complete(state, [rp.credential], damaged)

    emulator.restart()
    assert emulator.ctap.info.options["clientPin"] is True
    after, _, after_interaction = rp.authenticate(emulator, pin)
    assert after.raw_id == response.raw_id
    assert after_interaction.pin_requests == [(2, rp.id)], "new process must request a new PIN token"
    after_counter = after.response.authenticator_data.counter
    assert after_counter > counter, "this baseline's monotonic counter must survive a normal restart"
    print("restart " + json.dumps({"old_credential_verified": True,
                                   "counter_after_restart": after_counter}))
    emulator.check_store()


def test_policy_allowed_up_only_assertion(emulator):
    pin = set_pin(emulator)
    rp = RelyingParty()
    rp.register(emulator, pin, resident=RK.DISCOURAGED)
    response, state, interaction = rp.authenticate(
        emulator, None, discoverable=False, uv=UV.DISCOURAGED
    )
    assert interaction.pin_requests == [], "UP-only must not silently upgrade to PIN UV"
    assert not response.response.authenticator_data.is_user_verified()
    print("up-only " + json.dumps({"up": True, "uv": False,
                                   "terminal_prompts": emulator.prompts}))
    required_state = dict(state, user_verification=UV.REQUIRED)
    with pytest.raises(ValueError, match="^User verification required, but user verified flag not set\\.$"):
        rp.server.authenticate_complete(required_state, [rp.credential], response)
