// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! A pulse belongs to one explicitly armed FIDO window, never the next request.

use crate::presence::{SCOPE_CCID, SCOPE_FIDO};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

const FLAGS: u32 = 7;
const WAIT_LOW: u32 = State::WaitLow as u32;
const ARMED: u32 = State::Armed as u32;
const EVENT: u32 = State::Event as u32;
const SPENT: u32 = State::Spent as u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum State {
    Closed = 0,
    WaitLow = 1,
    Armed = 2,
    Event = 3,
    Spent = 4,
    Disabled = 5,
}

pub struct Touch {
    next: AtomicU32,
    state: AtomicU32,
    available: AtomicBool,
}

impl Default for Touch {
    fn default() -> Self {
        Self::new()
    }
}

impl Touch {
    pub const fn new() -> Self {
        Self::with_availability(true)
    }

    pub const fn new_disabled() -> Self {
        Self::with_availability(false)
    }

    const fn with_availability(available: bool) -> Self {
        Self {
            next: AtomicU32::new(0),
            state: AtomicU32::new(0),
            available: AtomicBool::new(available),
        }
    }

    pub fn enable(&self) {
        self.available.store(true, Ordering::Release);
    }

    pub fn disable(&self) {
        self.available.store(false, Ordering::Release);
        self.state.store(0, Ordering::Release);
    }

    pub fn window(&self) -> Option<u32> {
        let window = self.state.load(Ordering::Acquire) & !FLAGS;
        (window != 0).then_some(window)
    }

    pub fn snapshot(&self) -> (u32, State) {
        let state = self.state.load(Ordering::Acquire);
        let consent = if !self.available.load(Ordering::Acquire) {
            State::Disabled
        } else {
            match state & FLAGS {
                WAIT_LOW => State::WaitLow,
                ARMED => State::Armed,
                EVENT => State::Event,
                SPENT => State::Spent,
                _ => State::Closed,
            }
        };
        (state & !FLAGS, consent)
    }

    pub fn open(&self, scope: u8) -> Option<u32> {
        if !self.available.load(Ordering::Acquire) {
            return None;
        }
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
