// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;

#[kani::proof]
fn only_current_active_attempt_is_accepted() {
    let mut operations = Operations::new();
    let channel: u32 = kani::any();
    let attempt = operations.begin(channel, 0).unwrap();
    let observation = Attempt {
        id: kani::any(),
        channel: kani::any(),
        started_ms: kani::any(),
        deadline_ms: kani::any(),
    };
    let now: u64 = kani::any();
    assert_eq!(
        operations.accepts(observation, now),
        observation == attempt && now < VERIFY_TIMEOUT_MS
    );
}

#[kani::proof]
fn a_terminal_attempt_cannot_reopen_before_confirmed_drain() {
    let mut operations = Operations::new();
    let attempt = operations.begin(kani::any(), 0).unwrap();
    operations.finish(attempt, 0);
    assert!(!operations.accepts(attempt, 0));
    assert!(operations.begin(kani::any(), 0).is_err());
    let mut stale = attempt;
    stale.id = 0;
    assert!(!operations.drained(stale));
    operations.expire_drain(DRAIN_TIMEOUT_MS);
    assert_eq!(operations.state(), Lifecycle::Unavailable);
    assert!(!operations.drained(attempt));
    assert!(operations.begin(kani::any(), DRAIN_TIMEOUT_MS).is_err());
}
