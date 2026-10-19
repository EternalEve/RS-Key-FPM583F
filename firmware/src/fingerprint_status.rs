// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! A snapshot of adapter state and the last failure, without UART or storage access.

use core::sync::atomic::{AtomicU32, Ordering};

use rsk_fido::fingerprint::Failure;
use rsk_vendor::fingerprint::{DriverFault, STATUS_LEN, STATUS_VERSION, Stage, UartFault};

static UART: AtomicU32 = AtomicU32::new(0);
static DRIVER: AtomicU32 = AtomicU32::new(0);

pub fn supply_starting() {
    UART.store(Stage::SupplyStarting as u32, Ordering::Relaxed);
    DRIVER.store(0, Ordering::Relaxed);
}

pub fn stage(stage: Stage) {
    let current = UART.load(Ordering::Relaxed);
    UART.store((current & !0xff) | stage as u32, Ordering::Relaxed);
}

pub fn uart_fault(fault: UartFault) {
    let current = UART.load(Ordering::Relaxed);
    UART.store((current & 0xff) | ((fault as u32) << 8), Ordering::Relaxed);
}

pub fn driver_fault(command: [u8; 2], failure: Failure) {
    let fault = match failure {
        Failure::Unavailable => DriverFault::Unavailable,
        Failure::NoMatch => DriverFault::NoMatch,
        Failure::InputTimeout => DriverFault::InputTimeout,
        Failure::Timeout => DriverFault::Timeout,
        Failure::Declined => DriverFault::Declined,
        Failure::Communication => DriverFault::Communication,
    };
    DRIVER.store(
        fault as u32 | ((command[0] as u32) << 8) | ((command[1] as u32) << 16),
        Ordering::Relaxed,
    );
}

pub fn snapshot() -> [u8; STATUS_LEN] {
    let uart = UART.load(Ordering::Relaxed);
    let driver = DRIVER.load(Ordering::Relaxed);
    [
        STATUS_VERSION,
        uart as u8,
        (uart >> 8) as u8,
        driver as u8,
        (driver >> 8) as u8,
        (driver >> 16) as u8,
    ]
}
