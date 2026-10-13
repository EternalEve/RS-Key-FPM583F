// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! The interrupt executor observes pulses; only the worker opens consent windows.

use embassy_futures::select::{Either, select};
use embassy_rp::gpio::Input;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use rsk_device::touch::Touch;

static TOUCH: Touch = Touch::new();
static REARM: Signal<CriticalSectionRawMutex, Option<u32>> = Signal::new();

pub fn open(scope: u8) -> Option<u32> {
    let window = TOUCH.open(scope);
    REARM.signal(window);
    window
}

pub fn take(window: u32) -> bool {
    TOUCH.take(window)
}

pub fn close(window: u32) {
    TOUCH.close(window);
    REARM.signal(None);
}

#[embassy_executor::task]
pub async fn task(mut input: Input<'static>) {
    let mut next = REARM.wait().await;
    loop {
        let Some(window) = next else {
            next = REARM.wait().await;
            continue;
        };
        // Arming while already high never treats that level as a new pulse.
        match select(REARM.wait(), input.wait_for_low()).await {
            Either::First(rearm) => {
                next = rearm;
                continue;
            }
            Either::Second(()) => {}
        }
        if !TOUCH.released(window) {
            next = REARM.wait().await;
            continue;
        }
        match select(REARM.wait(), input.wait_for_rising_edge()).await {
            Either::First(rearm) => next = rearm,
            Either::Second(()) => {
                TOUCH.record(window);
                next = REARM.wait().await;
            }
        }
    }
}
