# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Standard client interaction and an RP that keeps only registered public data."""

import secrets

from fido2.client import DefaultClientDataCollector, Fido2Client, UserInteraction
from fido2.cose import ES256
from fido2.ctap2.pin import ClientPin
from fido2.server import Fido2Server
from fido2.webauthn import (
    PublicKeyCredentialParameters,
    PublicKeyCredentialRpEntity,
    PublicKeyCredentialType,
    PublicKeyCredentialUserEntity,
    ResidentKeyRequirement,
    UserVerificationRequirement,
)

from session import Reply

UV = UserVerificationRequirement
RK = ResidentKeyRequirement
PIN_ATTEMPT_LIMIT = 1


class Interaction(UserInteraction):
    def __init__(self, pin):
        self.pin = pin
        self.pin_requests = []
        self.prompt_count = 0
        self.cancel = None

    def request_pin(self, permissions, rp_id):
        self.pin_requests.append((int(permissions), rp_id))
        if len(self.pin_requests) > PIN_ATTEMPT_LIMIT:
            return None
        return self.pin

    def request_uv(self, permissions, rp_id):
        raise AssertionError("baseline must not request built-in UV")

    def prompt_up(self):
        self.prompt_count += 1
        if self.cancel is not None:
            self.cancel.set()


class RelyingParty:
    def __init__(self):
        self.id = f"{secrets.token_hex(8)}.example.com"
        self.origin = f"https://{self.id}"
        self.user = PublicKeyCredentialUserEntity(
            id=secrets.token_bytes(16), name="synthetic-user", display_name="Synthetic User"
        )
        self.server = Fido2Server(PublicKeyCredentialRpEntity(id=self.id, name="RS-Key test RP"))
        self.server.allowed_algorithms = [PublicKeyCredentialParameters(
            type=PublicKeyCredentialType.PUBLIC_KEY, alg=ES256.ALGORITHM
        )]
        self.credential = None

    def client(self, emu, interaction):
        with emu.connection.operation():
            client = Fido2Client(
                emu.device, DefaultClientDataCollector(self.origin), interaction
            )
        # The backend is private in 2.2.1; this assertion detects its silent CTAP1 fallback.
        assert client._backend.ctap2.device is emu.device, "CTAP2 backend required"
        return client

    def register(self, emu, pin, resident=RK.REQUIRED, uv=UV.REQUIRED):
        options, state = self.server.register_begin(
            self.user, resident_key_requirement=resident, user_verification=uv
        )
        interaction = Interaction(pin)
        client = self.client(emu, interaction)
        with emu.confirmations():
            response = client.make_credential(options.public_key)
        auth_data = self.server.register_complete(state, response)
        assert auth_data.credential_data.credential_id == response.raw_id
        assert auth_data.credential_data.public_key[3] == ES256.ALGORITHM
        assert auth_data.is_user_present()
        if uv == UV.REQUIRED:
            assert auth_data.is_user_verified()
        self.credential = auth_data.credential_data
        return response, auth_data, interaction

    def request(self, discoverable=True, uv=UV.REQUIRED):
        return self.server.authenticate_begin(
            None if discoverable else [self.credential], user_verification=uv
        )

    def authenticate(self, emu, pin, discoverable=True, uv=UV.REQUIRED,
                     reply=Reply.APPROVE):
        options, state = self.request(discoverable, uv)
        interaction = Interaction(pin)
        client = self.client(emu, interaction)
        with emu.confirmations(reply):
            selection = client.get_assertion(options.public_key)
            responses = [selection.get_response(index)
                         for index in range(len(selection.get_assertions()))]
            matches = [response for response in responses if response.raw_id == self.credential.credential_id]
            assert len(matches) == 1, "exactly the registered credential must be selected"
            response = matches[0]
        verified = self.server.authenticate_complete(state, [self.credential], response)
        assert verified == self.credential
        assert response.response.authenticator_data.is_user_present()
        if discoverable:
            assert response.response.user_handle == self.user.id
        if uv == UV.REQUIRED:
            assert response.response.authenticator_data.is_user_verified()
        return response, state, interaction


def set_pin(emu):
    pin = secrets.token_hex(6)
    with emu.confirmations():
        client_pin = ClientPin(emu.ctap)
        assert client_pin.protocol.VERSION == 2
        client_pin.set_pin(pin)
        info = emu.ctap.get_info()
    assert info.options["clientPin"] is True
    assert info.options.get("uv") is not True
    return pin
