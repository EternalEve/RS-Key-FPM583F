// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;
use crate::presence::{SCOPE_NONE, SCOPE_OTP};

#[test]
fn old_idle_duplicate_and_cross_transport_events_do_not_confirm_a_new_window() {
    let touch = Touch::new();
    assert!(!touch.record(0));
    assert!(touch.open(SCOPE_NONE).is_none());
    assert!(touch.open(SCOPE_OTP).is_none());
    let first = touch.open(SCOPE_FIDO).unwrap();
    assert!(
        !touch.record(first),
        "a level already high at arm is not a new event"
    );
    assert!(touch.released(first));
    assert!(touch.record(first));
    assert!(!touch.record(first));
    let second = touch.open(SCOPE_CCID).unwrap();
    assert!(
        !touch.take(first),
        "an old event must not confirm a new transport's request"
    );
    assert!(
        !touch.record(first),
        "a delayed interrupt must not enter the next window"
    );
    touch.close(first);
    assert!(!touch.take(second));
    assert!(
        !touch.record(second),
        "an unbroken hold cannot confirm the next request"
    );
    assert!(touch.released(second));
    assert!(touch.record(second));
    assert!(
        !touch.take(first),
        "an old window must not consume the new transport's pulse"
    );
    assert!(touch.take(second));
    assert!(!touch.take(second));
    assert!(
        !touch.record(second),
        "repeat pulses in the same window cannot replenish consent"
    );
    touch.close(second);
    assert!(!touch.record(second));
}

#[test]
fn window_exhaustion_fails_closed_without_reusing_an_old_generation() {
    let touch = Touch::new();
    touch.next.store(!FLAGS, Ordering::Relaxed);
    assert!(touch.open(SCOPE_FIDO).is_none());
    assert!(!touch.record(0));
}
