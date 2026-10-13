// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;

#[kani::proof]
fn a_closed_or_consumed_pulse_cannot_confirm_a_new_window() {
    let touch = Touch::new();
    let first = touch.open(SCOPE_FIDO).unwrap();
    assert_eq!(first, FLAGS + 1);
    assert_eq!(touch.state.load(Ordering::Acquire), first | WAIT_LOW);
    assert!(!touch.record(first));
    assert!(touch.released(first));
    if kani::any() {
        assert!(touch.record(first));
    }
    touch.close(first);
    let second = touch.open(SCOPE_CCID).unwrap();
    assert!(!touch.record(first));
    assert!(!touch.take(first));
    assert!(!touch.take(second));
    assert!(!touch.record(second));
    assert!(touch.released(second));
    assert!(touch.record(second));
    assert!(!touch.take(first));
    assert!(touch.take(second));
    assert!(!touch.record(second));
    assert!(!touch.take(second));
}
