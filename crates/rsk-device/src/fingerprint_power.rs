// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Power changes require both a released business hold and a drained operation.

pub const BOOT_TIMEOUT_MS: u64 = 3_000;
pub const STOP_TIMEOUT_MS: u64 = 3_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Off,
    Starting,
    Running,
    Stopping,
    Unavailable,
}

pub struct Power {
    pub(crate) phase: Phase,
    pub(crate) deadline_ms: u64,
    controlled: bool,
    held: bool,
}

impl Power {
    pub fn new(controlled: bool, now_ms: u64) -> Self {
        Self {
            phase: if controlled {
                Phase::Off
            } else {
                Phase::Starting
            },
            deadline_ms: now_ms.saturating_add(BOOT_TIMEOUT_MS),
            controlled,
            held: false,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn wake(&mut self, now_ms: u64, deadline_ms: u64) -> bool {
        self.held = true;
        if self.phase != Phase::Off || now_ms >= deadline_ms {
            return false;
        }
        self.phase = Phase::Starting;
        self.deadline_ms = deadline_ms.min(now_ms.saturating_add(BOOT_TIMEOUT_MS));
        true
    }

    pub fn started(&mut self) -> bool {
        if self.phase != Phase::Starting {
            return false;
        }
        self.phase = Phase::Running;
        true
    }

    pub fn release(&mut self, enrollment: bool) {
        self.held = enrollment;
    }

    pub fn stop(&mut self, now_ms: u64, drained: bool) -> bool {
        if !self.controlled || self.held || !drained || self.phase != Phase::Running {
            return false;
        }
        self.phase = Phase::Stopping;
        self.deadline_ms = now_ms.saturating_add(STOP_TIMEOUT_MS);
        true
    }

    pub fn stopped(&mut self) -> bool {
        if self.phase != Phase::Stopping {
            return false;
        }
        self.phase = Phase::Off;
        true
    }

    pub fn fail(&mut self) {
        self.phase = Phase::Unavailable;
    }
}

#[cfg(test)]
#[path = "fingerprint_power_tests.rs"]
mod tests;

#[cfg(kani)]
#[path = "fingerprint_power_kani.rs"]
mod proofs;
