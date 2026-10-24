// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;
use crate::presence::SCOPE_FIDO;

#[kani::proof]
fn maintenance_and_old_edges_never_grant_new_consent() {
    let touch = Rearm::new();
    assert!(touch.open(SCOPE_FIDO).is_none());
    touch.enable();
    let first = touch.open(SCOPE_FIDO).unwrap();
    touch.observe(Some(first), false);
    touch.observe(Some(first), true);
    assert!(touch.take(first));
    touch.close(first);
    let next = touch.open(SCOPE_FIDO).unwrap();
    touch.observe(Some(next), true);
    assert!(!touch.take(next));
    touch.observe(Some(next), false);
    assert!(touch.transition(Phase::Pending, Phase::Running));
    touch.observe(Some(next), kani::any());
    assert!(!touch.take(next));
    assert!(!touch.take(first));
    assert!(touch.transition(Phase::Running, Phase::Settling));
    touch.observe(Some(next), true);
    assert!(!touch.take(next));
    touch.observe(Some(first), false);
    touch.observe(Some(next), true);
    assert!(!touch.take(next));
}
