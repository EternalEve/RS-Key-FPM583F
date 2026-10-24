// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Measurement-only RAM observations; UART and power sequencing stay with the worker.

use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, Ordering};

use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_time::Instant;
use rsk_vendor::fingerprint_trace::{Event, Level, PAGE_LEN, Snapshot, Trace};

static TRACE: Mutex<CriticalSectionRawMutex, RefCell<Trace>> =
    Mutex::new(RefCell::new(Trace::new()));
static INPUT_READY: AtomicBool = AtomicBool::new(false);

fn level() -> Level {
    if !INPUT_READY.load(Ordering::Acquire) {
        return Level::Unknown;
    }
    let pin = crate::env_u16(env!("PK_FINGERPRINT_TOUCH_PIN"));
    if rp_pac::SIO.gpio_in((pin / 32) as usize).read() & (1 << (pin % 32)) == 0 {
        Level::Low
    } else {
        Level::High
    }
}

fn snapshot(level: Level) -> Snapshot {
    let (window, consent) = crate::fingerprint_touch::consent_snapshot();
    Snapshot {
        phase: crate::fingerprint_touch::phase() as u8,
        consent: consent as u8,
        level,
        window,
        adapter_stage: crate::fingerprint::status::snapshot()[1],
    }
}

pub fn started() {
    INPUT_READY.store(true, Ordering::Release);
}

pub fn record(event: Event, observed: Option<bool>, window: Option<u32>) {
    TRACE.lock(|trace| {
        let level = match observed {
            Some(false) => Level::Low,
            Some(true) => Level::High,
            None => level(),
        };
        trace.borrow_mut().record(
            Instant::now().as_millis() as u32,
            event,
            Snapshot {
                window: window.unwrap_or(0),
                ..snapshot(level)
            },
        );
    });
}

pub fn page(page: u8) -> Option<[u8; PAGE_LEN]> {
    TRACE.lock(|trace| trace.borrow().page(page, snapshot(level())))
}
