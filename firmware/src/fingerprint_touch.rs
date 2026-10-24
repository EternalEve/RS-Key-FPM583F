// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! The interrupt executor observes pulses; only the worker opens consent windows.

use embassy_futures::select::{Either, select};
use embassy_rp::gpio::Input;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
#[cfg(fingerprint_power)]
use rsk_device::fingerprint_touch::Rearm;
#[cfg(not(fingerprint_power))]
use rsk_device::touch::Touch;
#[cfg(feature = "fingerprint-touch-trace")]
use rsk_vendor::fingerprint_trace::Event;

#[cfg(fingerprint_power)]
static TOUCH: Rearm = Rearm::new();
#[cfg(not(fingerprint_power))]
static TOUCH: Touch = Touch::new();
static REARM: Signal<CriticalSectionRawMutex, Option<u32>> = Signal::new();

#[cfg(fingerprint_power)]
pub fn initialized() {
    TOUCH.enable();
    #[cfg(feature = "fingerprint-touch-trace")]
    crate::fingerprint_touch_trace::record(Event::Initialized, None, None);
}

#[cfg(feature = "fingerprint-touch-trace")]
pub fn phase() -> rsk_device::fingerprint_touch::Phase {
    TOUCH.phase()
}

#[cfg(feature = "fingerprint-touch-trace")]
pub fn consent_snapshot() -> (u32, rsk_device::touch::State) {
    TOUCH.consent_snapshot()
}

pub fn open(scope: u8) -> Option<u32> {
    let window = TOUCH.open(scope);
    REARM.signal(window);
    #[cfg(feature = "fingerprint-touch-trace")]
    crate::fingerprint_touch_trace::record(Event::Open, None, window);
    window
}

pub fn take(window: u32) -> bool {
    let taken = TOUCH.take(window);
    #[cfg(feature = "fingerprint-touch-trace")]
    if taken {
        crate::fingerprint_touch_trace::record(Event::Taken, None, Some(window));
    }
    taken
}

pub fn close(window: u32) {
    TOUCH.close(window);
    REARM.signal(None);
    #[cfg(feature = "fingerprint-touch-trace")]
    crate::fingerprint_touch_trace::record(Event::Close, None, Some(window));
}

#[cfg(fingerprint_power)]
pub fn service(sensor: &core::cell::RefCell<crate::fingerprint::Bridge>) {
    #[cfg(feature = "fingerprint-touch-trace")]
    let before = TOUCH.phase();
    if TOUCH.service(&mut sensor.borrow_mut()) {
        REARM.signal(TOUCH.window());
    }
    #[cfg(feature = "fingerprint-touch-trace")]
    if TOUCH.phase() != before {
        crate::fingerprint_touch_trace::record(Event::Service, None, TOUCH.window());
    }
}

#[cfg(fingerprint_power)]
#[embassy_executor::task]
pub async fn task(mut input: Input<'static>) {
    #[cfg(feature = "fingerprint-touch-trace")]
    crate::fingerprint_touch_trace::started();
    let high = input.is_high();
    TOUCH.observe(None, high);
    #[cfg(feature = "fingerprint-touch-trace")]
    crate::fingerprint_touch_trace::record(Event::Resample, Some(high), None);
    loop {
        let window = TOUCH.window();
        match select(REARM.wait(), input.wait_for_any_edge()).await {
            Either::First(_) => {
                let window = if input.is_high() {
                    None
                } else {
                    TOUCH.window()
                };
                let high = input.is_high();
                TOUCH.observe(window, high);
                #[cfg(feature = "fingerprint-touch-trace")]
                crate::fingerprint_touch_trace::record(Event::Resample, Some(high), window);
            }
            Either::Second(()) => {
                let high = input.is_high();
                TOUCH.observe(window, high);
                #[cfg(feature = "fingerprint-touch-trace")]
                crate::fingerprint_touch_trace::record(Event::Edge, Some(high), window);
            }
        }
    }
}

#[cfg(not(fingerprint_power))]
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
