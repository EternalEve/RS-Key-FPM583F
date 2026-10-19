// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;
use crate::tests::{Env, apdu, select, sw};

#[test]
fn fido_ccid_switch_controls_selection_without_changing_hid_or_other_applets() {
    let env = Env::new();
    let mut hid = env.ctap();
    let mut ccid = env.ccid();
    let expected = if cfg!(feature = "fido-ccid") {
        Sw::OK
    } else {
        Sw::FILE_NOT_FOUND
    };
    for aid in [rsk_fido::consts::FIDO_AID, &rsk_fido::consts::FIDO_AID[..5]] {
        assert_eq!(sw(ccid.handle_apdu(&select(aid), 0)), expected);
    }
    assert_eq!(
        hid.handle_cbor(1, &[rsk_fido::consts::CTAP_GET_INFO], 0)[0],
        0
    );
    let version = apdu(0, rsk_fido::consts::CTAP_VERSION, 0, 0, &[]);
    assert_eq!(sw(hid.handle_msg(1, &version, 0)), Sw::OK);
    for aid in [
        rsk_vendor::VENDOR_AID,
        rsk_openpgp::consts::OPENPGP_AID,
        rsk_mgmt::MANAGEMENT_AID,
        rsk_oath::OATH_AID,
        rsk_otp::OTP_AID,
        rsk_piv::PIV_AID,
        rsk_rescue::RESCUE_AID,
    ] {
        assert_eq!(sw(ccid.handle_apdu(&select(aid), 0)), Sw::OK);
    }
}

#[cfg(not(feature = "fido-ccid"))]
#[test]
fn enabling_all_applications_cannot_open_the_disabled_fido_ccid_route() {
    let env = Env::new();
    let mut ccid = env.ccid();
    let blob = crate::tests::dev_conf(rsk_devconf::SUPPORTED_CAPS);
    rsk_devconf::persist_dev_conf(&mut env.fs.borrow_mut(), &blob[1..]).unwrap();
    ccid.refresh_enabled();
    for _ in 0..2 {
        assert_eq!(
            sw(ccid.handle_apdu(&select(rsk_fido::consts::FIDO_AID), 0)),
            Sw::FILE_NOT_FOUND
        );
        let generation = env.fs.borrow().write_gen();
        for command in [
            apdu(0x80, 0x10, 0, 0, &[rsk_fido::consts::CTAP_RESET]),
            apdu(0, rsk_fido::consts::CTAP_VERSION, 0, 0, &[]),
        ] {
            assert_ne!(sw(ccid.handle_apdu(&command, 0)), Sw::OK);
        }
        assert_eq!(env.fs.borrow().write_gen(), generation);
        ccid.reset_card();
    }
}
