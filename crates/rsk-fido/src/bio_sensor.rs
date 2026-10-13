// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Module facts for management, sharing verification's exclusive operation path.

use core::task::Poll;

use crate::CtapError;
use crate::fingerprint::{Attempt, Failure, Sensor, VERIFY_TIMEOUT_MS};

pub const MODULE_SLOTS: usize = 100;
pub const INVENTORY_BYTES: usize = MODULE_SLOTS.div_ceil(8);
pub const SESSION_TIMEOUT_MS: u64 = VERIFY_TIMEOUT_MS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub samples: u8,
    pub slots: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Inventory(pub [u8; INVENTORY_BYTES]);

impl Inventory {
    pub fn contains(&self, slot: u16) -> bool {
        let slot = slot as usize;
        slot < MODULE_SLOTS && self.0[slot / 8] & (1 << (slot % 8)) != 0
    }
    pub fn set(&mut self, slot: u16, present: bool) {
        let slot = slot as usize;
        if slot < MODULE_SLOTS {
            if present {
                self.0[slot / 8] |= 1 << (slot % 8);
            } else {
                self.0[slot / 8] &= !(1 << (slot % 8));
            }
        }
    }
    pub fn empty(&self) -> bool {
        self.0.iter().all(|b| *b == 0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Capture {
        slot: u16,
        generation: u32,
        first: bool,
        remaining: u8,
    },
    Delete {
        slot: u16,
    },
    Clear,
    Inventory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Feedback {
    Good = 0,
    TooHigh = 1,
    TooLow = 2,
    TooLeft = 3,
    TooRight = 4,
    TooFast = 5,
    TooSlow = 6,
    PoorQuality = 7,
    TooSkewed = 8,
    TooShort = 9,
    MergeFailure = 10,
    Exists = 11,
    NoActivity = 13,
    NoTransition = 14,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Result {
    Sample {
        feedback: Feedback,
        remaining: u8,
        saved: bool,
    },
    Deleted,
    Cleared,
    Inventory(Inventory),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Observation {
    pub attempt: Attempt,
    pub result: core::result::Result<Result, Failure>,
}

pub fn execute(
    sensor: &mut dyn Sensor,
    channel: u32,
    command: Command,
    timeout_ms: u64,
) -> core::result::Result<Result, CtapError> {
    let deadline_ms = sensor.now_ms().saturating_add(timeout_ms);
    execute_until(sensor, channel, command, deadline_ms)
}

pub(crate) fn execute_until(
    sensor: &mut dyn Sensor,
    channel: u32,
    command: Command,
    deadline: u64,
) -> core::result::Result<Result, CtapError> {
    if sensor.cancelled() {
        return Err(CtapError::KeepAliveCancel);
    }
    sensor.prepare(deadline)?;
    let attempt = sensor
        .start_management(channel, command)
        .map_err(Failure::ctap)?;
    let deadline = attempt.deadline_ms.min(sensor.deadline_ms(deadline));
    let result = wait(sensor, attempt, deadline);
    sensor.finish(attempt);
    result
}

fn wait(
    sensor: &mut dyn Sensor,
    attempt: Attempt,
    deadline: u64,
) -> core::result::Result<Result, CtapError> {
    loop {
        if sensor.cancelled() {
            return Err(CtapError::KeepAliveCancel);
        }
        let now = sensor.now_ms();
        if now < attempt.started_ms || now >= deadline {
            return Err(CtapError::UserActionTimeout);
        }
        match sensor.poll_management() {
            Poll::Pending => sensor.wait(),
            Poll::Ready(observation) => {
                if observation.attempt != attempt {
                    return Err(CtapError::Other);
                }
                if sensor.cancelled() {
                    return Err(CtapError::KeepAliveCancel);
                }
                if sensor.now_ms() >= deadline || sensor.now_ms() < attempt.started_ms {
                    return Err(CtapError::UserActionTimeout);
                }
                return observation.result.map_err(Failure::ctap);
            }
        }
    }
}

pub fn read_inventory(
    sensor: &mut dyn Sensor,
    channel: u32,
) -> core::result::Result<Inventory, CtapError> {
    match execute(sensor, channel, Command::Inventory, SESSION_TIMEOUT_MS)? {
        Result::Inventory(inventory) => Ok(inventory),
        _ => Err(CtapError::Other),
    }
}
