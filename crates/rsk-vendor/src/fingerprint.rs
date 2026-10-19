// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Non-secret, volatile fingerprint startup diagnostics; reads never start the module.

pub const STATUS_LEN: usize = 6;
pub const STATUS_VERSION: u8 = 1;

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum Stage {
    Off = 0,
    SupplyStarting = 1,
    UartStarting = 2,
    UartReady = 3,
}

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum UartFault {
    None = 0,
    Reset = 1,
    Overrun = 2,
    Break = 3,
    Parity = 4,
    Framing = 5,
    Inactive = 6,
    Unknown = 7,
}

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum DriverFault {
    None = 0,
    Unavailable = 1,
    NoMatch = 2,
    InputTimeout = 3,
    Timeout = 4,
    Declined = 5,
    Communication = 6,
}
