// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! TOUCHOUT consent and ordinary-sleep maintenance have separate lifetimes.

use core::sync::atomic::{AtomicU8, Ordering};

use crate::fingerprint_driver::{Driver, Runtime};
use crate::fingerprint_io::Stream;
use crate::fingerprint_power::Phase as PowerPhase;
use crate::touch::Touch;
use rsk_fido::fingerprint::Sensor;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Phase {
    Cold = 0,
    Ready = 1,
    High = 2,
    Pending = 3,
    Running = 4,
    Settling = 5,
    Disabled = 6,
}

pub struct Rearm {
    touch: Touch,
    phase: AtomicU8,
}

impl Default for Rearm {
    fn default() -> Self {
        Self::new()
    }
}

impl Rearm {
    pub const fn new() -> Self {
        Self {
            touch: Touch::new_disabled(),
            phase: AtomicU8::new(Phase::Cold as u8),
        }
    }

    pub fn enable(&self) {
        if self.transition(Phase::Cold, Phase::Ready) {
            self.touch.enable();
        }
    }

    pub fn phase(&self) -> Phase {
        match self.phase.load(Ordering::Acquire) {
            n if n == Phase::Cold as u8 => Phase::Cold,
            n if n == Phase::Ready as u8 => Phase::Ready,
            n if n == Phase::High as u8 => Phase::High,
            n if n == Phase::Pending as u8 => Phase::Pending,
            n if n == Phase::Running as u8 => Phase::Running,
            n if n == Phase::Settling as u8 => Phase::Settling,
            _ => Phase::Disabled,
        }
    }

    fn transition(&self, from: Phase, to: Phase) -> bool {
        self.phase
            .compare_exchange(from as u8, to as u8, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub fn open(&self, scope: u8) -> Option<u32> {
        self.touch.open(scope)
    }

    pub fn window(&self) -> Option<u32> {
        self.touch.window()
    }

    pub fn consent_snapshot(&self) -> (u32, crate::touch::State) {
        self.touch.snapshot()
    }

    pub fn take(&self, window: u32) -> bool {
        self.touch.take(window)
    }

    pub fn close(&self, window: u32) {
        self.touch.close(window);
    }

    /// `window` was captured before waiting for the edge; None never grants consent.
    pub fn observe(&self, window: Option<u32>, high: bool) {
        loop {
            let phase = self.phase();
            let next = match (phase, high) {
                (Phase::Ready | Phase::Pending, true) => Phase::High,
                (Phase::High, false) => Phase::Pending,
                (Phase::Settling, false) => Phase::Ready,
                _ => phase,
            };
            if !self.transition(phase, next) {
                continue;
            }
            if let Some(window) = window {
                if phase == Phase::Ready && high {
                    self.touch.record(window);
                } else if matches!(phase, Phase::Ready | Phase::Settling) && !high {
                    self.touch.released(window);
                }
            }
            return;
        }
    }

    /// One bounded worker step; never borrows the UART from an interrupt task.
    pub fn service<U: Stream, C: Runtime>(&self, driver: &mut Driver<U, C>) -> bool {
        if driver.power_phase() == PowerPhase::Unavailable {
            self.phase.store(Phase::Disabled as u8, Ordering::Release);
            self.touch.disable();
            return false;
        }
        if driver.touch_idle() && self.transition(Phase::Pending, Phase::Running) {
            driver.start_touch_cycle();
        }
        if self.phase() == Phase::Running {
            if driver.touch_running() {
                driver.service_idle();
            }
            if driver.power_phase() == PowerPhase::Unavailable {
                self.phase.store(Phase::Disabled as u8, Ordering::Release);
                self.touch.disable();
            } else if !driver.touch_running() {
                // Switched-rail edges belong to this cycle, not another touch.
                return self.transition(Phase::Running, Phase::Settling);
            }
        }
        false
    }
}

#[cfg(test)]
#[path = "fingerprint_touch_tests.rs"]
mod tests;

#[cfg(kani)]
#[path = "fingerprint_touch_kani.rs"]
mod proofs;
