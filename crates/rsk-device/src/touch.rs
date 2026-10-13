// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! A pulse belongs to one explicitly armed FIDO window, never the next request.

use crate::presence::{SCOPE_CCID, SCOPE_FIDO};
use core::sync::atomic::{AtomicU32, Ordering};

const FLAGS: u32 = 7;
const WAIT_LOW: u32 = 1;
const ARMED: u32 = 2;
const EVENT: u32 = 3;
const SPENT: u32 = 4;

pub struct Touch {
    next: AtomicU32,
    state: AtomicU32,
}

impl Default for Touch {
    fn default() -> Self {
        Self::new()
    }
}

impl Touch {
    pub const fn new() -> Self {
        Self {
            next: AtomicU32::new(0),
            state: AtomicU32::new(0),
        }
    }

    pub fn open(&self, scope: u8) -> Option<u32> {
        self.state.store(0, Ordering::Release);
        if !matches!(scope, SCOPE_FIDO | SCOPE_CCID) {
            return None;
        }
        let old = self.next.load(Ordering::Acquire);
        let window = old.checked_add(FLAGS + 1)?;
        self.next
            .compare_exchange(old, window, Ordering::AcqRel, Ordering::Acquire)
            .ok()?;
        self.state.store(window | WAIT_LOW, Ordering::Release);
        Some(window)
    }

    pub fn record(&self, window: u32) -> bool {
        self.transition(window, ARMED, EVENT)
    }

    pub fn released(&self, window: u32) -> bool {
        self.transition(window, WAIT_LOW, ARMED)
    }

    pub fn take(&self, window: u32) -> bool {
        self.transition(window, EVENT, SPENT)
    }

    fn transition(&self, window: u32, from: u32, to: u32) -> bool {
        let expected = window | from;
        // Kani 0.67 over-approximates the CAS success flag; retain the observed value.
        self.state
            .compare_exchange(expected, window | to, Ordering::AcqRel, Ordering::Acquire)
            .is_ok_and(|observed| observed == expected)
    }

    pub fn close(&self, window: u32) {
        let state = self.state.load(Ordering::Acquire);
        if state & !FLAGS == window {
            let _ = self
                .state
                .compare_exchange(state, 0, Ordering::AcqRel, Ordering::Acquire);
        }
    }
}

#[cfg(test)]
#[path = "touch_tests.rs"]
mod tests;

#[cfg(kani)]
#[path = "touch_kani.rs"]
mod proofs;
