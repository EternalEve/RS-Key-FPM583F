// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;

#[test]
fn only_a_released_drained_running_module_can_stop() {
    for enrollment in [true, false] {
        for drained in [true, false] {
            let mut power = Power::new(true, 0);
            assert!(!power.stop(0, drained));
            assert!(power.wake(1, 1_000));
            assert!(!power.stop(1, drained));
            assert!(power.started());
            assert!(!power.stop(2, drained));
            power.release(enrollment);
            assert_eq!(power.stop(2, drained), !enrollment && drained);
        }
    }
}

#[test]
fn cycles_preserve_deadlines_and_faults_cannot_restart() {
    let mut power = Power::new(true, 0);
    assert!(!power.started());
    assert!(!power.stopped());
    for now in [1, 100, 10_000] {
        assert!(power.wake(now, now + 500));
        assert_eq!(power.deadline_ms, now + 500);
        assert!(power.started());
        power.release(false);
        assert!(power.stop(now + 1, true));
        assert!(!power.wake(now + 2, now + 1_000));
        power.release(false);
        assert!(power.stopped());
    }
    assert!(!power.wake(10, 10));
    assert_eq!(power.phase(), Phase::Off);
    power.fail();
    assert!(!power.wake(11, 500));
    assert!(!power.started());
    assert!(!power.stopped());
    assert!(!power.stop(12, true));
    assert_eq!(power.phase(), Phase::Unavailable);
}

#[test]
fn the_constant_supply_profile_never_requests_a_power_cut() {
    let mut power = Power::new(false, 0);
    assert!(power.started());
    power.release(false);
    assert!(!power.stop(1, true));
    assert_eq!(power.phase(), Phase::Running);
}
