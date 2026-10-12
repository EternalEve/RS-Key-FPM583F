// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;
use crate::tests::{Env, apdu, select, sw};
use core::task::Poll;
use rsk_fido::fingerprint::{Attempt, Failure, Matching, Observation, Sensor};

struct Unavailable;
impl Sensor for Unavailable {
    fn now_ms(&self) -> u64 {
        0
    }
    fn ready(&self) -> bool {
        false
    }
    fn start(&mut self, _: u32) -> Result<Attempt, Failure> {
        Err(Failure::Unavailable)
    }
    fn poll(&mut self) -> Poll<Observation> {
        Poll::Pending
    }
    fn cancelled(&self) -> bool {
        false
    }
    fn matching(&self) -> Matching {
        Matching::NotStarted
    }
    fn wait(&mut self) {}
    fn finish(&mut self, _: Attempt) {}
    fn service_idle(&mut self) {}
}

#[test]
fn fingerprint_management_aliases_never_acknowledge_or_request_presence() {
    let env = Env::new();
    let sensor = RefCell::new(Unavailable);
    let bindings = RefCell::new(rsk_fido::bio_directory::Directory);
    let mut ccid = env.ccid();
    ccid.set_fingerprint(&sensor, &bindings);
    assert_eq!(
        sw(ccid.handle_apdu(&select(rsk_mgmt::MANAGEMENT_AID), 0)),
        rsk_sdk::Sw::OK
    );
    let before = env.finger.borrow().requests;
    for ins in [0x1E, 0x1F] {
        assert_eq!(
            sw(ccid.handle_apdu(&apdu(0, ins, 0, 0, &[]), 0)),
            rsk_sdk::Sw::INS_NOT_SUPPORTED
        );
    }
    assert_eq!(env.finger.borrow().requests, before);
    #[cfg(not(feature = "strict-config"))]
    assert!(
        !ccid.factory_wipe(),
        "the deferred shared entry must also be unreachable"
    );
}

#[cfg(not(feature = "strict-config"))]
#[test]
fn a_disabled_fingerprint_build_cannot_wipe_retained_isolation() {
    let env = Env::new();
    rsk_fido::bio_directory::begin_reset(&mut env.fs.borrow_mut()).unwrap();
    let mut ccid = env.ccid();
    assert!(!ccid.factory_wipe());
    assert_eq!(
        sw(ccid.handle_apdu(&select(rsk_mgmt::MANAGEMENT_AID), 0)),
        rsk_sdk::Sw::OK
    );
    for ins in [0x1E, 0x1F] {
        assert_eq!(
            sw(ccid.handle_apdu(&apdu(0, ins, 0, 0, &[]), 0)),
            rsk_sdk::Sw::INS_NOT_SUPPORTED
        );
    }
    assert!(rsk_fido::bio_directory::reset_pending(&mut env.fs.borrow_mut()).unwrap());
}
