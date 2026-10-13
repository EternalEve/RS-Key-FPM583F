// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;

#[kani::proof]
fn a_hold_or_undrained_operation_prevents_shutdown() {
    let enrollment: bool = kani::any();
    let drained: bool = kani::any();
    let mut power = Power::new(true, 0);
    assert!(power.wake(0, BOOT_TIMEOUT_MS));
    assert!(power.started());
    assert!(!power.stop(1, drained));
    power.release(enrollment);
    assert_eq!(power.stop(1, drained), !enrollment && drained);
    power.fail();
    assert!(!power.wake(2, BOOT_TIMEOUT_MS));
    assert!(!power.stopped());
}
