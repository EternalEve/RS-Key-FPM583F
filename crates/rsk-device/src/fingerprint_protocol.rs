// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! FPM383C/583F V1.2.0, pp10–12, 16–27 and 45; not the EF01 protocol.

use rsk_fido::bio_sensor::MODULE_SLOTS;

pub const HEADER: [u8; 8] = [0xf1, 0x1f, 0xe2, 0x2e, 0xb6, 0x6b, 0xa8, 0x8a];
pub const LINK_BYTES: usize = HEADER.len() + 3;
pub const MAX_FRAME: usize = 88;
pub const MAX_APPLICATION: usize = MAX_FRAME - LINK_BYTES;
pub const ENROLL_SAMPLES: u8 = 6;
const REQUEST_BYTES: usize = 7;
const RESPONSE_BYTES: usize = 11;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    Capture(u8),
    CaptureResult,
    Save(u16),
    SaveResult,
    Cancel,
    Match,
    MatchResult,
    Delete(u16),
    Clear,
    DeleteResult,
    Slot(u16),
    Finger,
    Count,
    Sleep,
}

impl Request {
    pub fn command(self) -> [u8; 2] {
        match self {
            Self::Capture(_) => [1, 0x11],
            Self::CaptureResult => [1, 0x12],
            Self::Save(_) => [1, 0x13],
            Self::SaveResult => [1, 0x14],
            Self::Cancel => [1, 0x15],
            Self::Match => [1, 0x21],
            Self::MatchResult => [1, 0x22],
            Self::Delete(_) | Self::Clear => [1, 0x31],
            Self::DeleteResult => [1, 0x32],
            Self::Slot(_) => [1, 0x33],
            Self::Finger => [1, 0x35],
            Self::Count => [2, 3],
            Self::Sleep => [2, 0x0c],
        }
    }

    pub fn encode(self, password: [u8; 4], out: &mut [u8]) -> Result<usize, Error> {
        let mut params = [0; 3];
        let n = match self {
            Self::Sleep => 1,
            Self::Capture(n) if (1..=ENROLL_SAMPLES).contains(&n) => {
                params[0] = n;
                1
            }
            Self::Capture(_) => return Err(Error::Parameter),
            Self::Save(slot) | Self::Slot(slot) if slot < MODULE_SLOTS as u16 => {
                params[..2].copy_from_slice(&slot.to_be_bytes());
                2
            }
            Self::Save(_) | Self::Slot(_) => return Err(Error::Parameter),
            Self::Delete(slot) if slot < MODULE_SLOTS as u16 => {
                params[1..].copy_from_slice(&slot.to_be_bytes());
                3
            }
            Self::Delete(_) => return Err(Error::Parameter),
            Self::Clear => {
                params[0] = 1;
                3
            }
            _ => 0,
        };
        let application = REQUEST_BYTES + n;
        let length = LINK_BYTES + application;
        if out.len() < length {
            return Err(Error::Length);
        }
        out[..HEADER.len()].copy_from_slice(&HEADER);
        out[8..10].copy_from_slice(&(application as u16).to_be_bytes());
        out[10] = checksum(&out[..10]);
        out[11..15].copy_from_slice(&password);
        out[15..17].copy_from_slice(&self.command());
        out[17..17 + n].copy_from_slice(&params[..n]);
        out[length - 1] = checksum(&out[LINK_BYTES..length - 1]);
        Ok(length)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Noise,
    Length,
    HeaderChecksum,
    ApplicationChecksum,
    Password,
    Command,
    Parameter,
    Completion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Completion(pub u8);

impl Completion {
    pub const OK: Self = Self(0);
    pub const BUSY: Self = Self(4);
    pub const NO_REQUEST: Self = Self(5);
    pub const NO_FINGER: Self = Self(8);
    pub const INTERRUPTED: Self = Self(0x15);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Data {
    None,
    Capture {
        slot: u16,
        progress: u8,
    },
    Saved(u16),
    Match {
        matched: bool,
        score: u16,
        slot: u16,
    },
    Slot {
        present: bool,
        slot: u16,
    },
    Finger(bool),
    Count(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reply {
    pub completion: Completion,
    pub data: Data,
}

#[derive(Clone, Copy)]
pub struct Frame {
    bytes: [u8; MAX_APPLICATION],
    len: usize,
}

impl Frame {
    pub fn decode(&self, request: Request, password: [u8; 4]) -> Result<Reply, Error> {
        let bytes = &self.bytes[..self.len];
        if bytes.len() < RESPONSE_BYTES {
            return Err(Error::Length);
        }
        if bytes[..4] != password {
            return Err(Error::Password);
        }
        if bytes[4..6] != request.command() {
            return Err(Error::Command);
        }
        let code = u32::from_be_bytes(bytes[6..10].try_into().map_err(|_| Error::Length)?);
        if code > 0x1a && code != 0xff {
            return Err(Error::Completion);
        }
        let completion = Completion(code as u8);
        let data = &bytes[10..bytes.len() - 1];
        let expected = match request {
            Request::CaptureResult | Request::Slot(_) => 3,
            Request::SaveResult | Request::Count => 2,
            Request::MatchResult => 6,
            Request::Finger => 1,
            _ => 0,
        };
        if completion != Completion::OK {
            // p22 has a short busy reply; error payloads never become module facts.
            if !data.is_empty() && data.len() != expected {
                return Err(Error::Length);
            }
            return Ok(Reply {
                completion,
                data: Data::None,
            });
        }
        if data.len() != expected {
            return Err(Error::Length);
        }
        let slot = |at: usize| u16::from_be_bytes([data[at], data[at + 1]]);
        let data = match request {
            Request::CaptureResult if slot(0) < MODULE_SLOTS as u16 => Data::Capture {
                slot: slot(0),
                progress: data[2],
            },
            Request::CaptureResult => return Err(Error::Parameter),
            Request::SaveResult => Data::Saved(slot(0)),
            Request::MatchResult if data[..2] == [0, 0] || data[..2] == [0, 1] => Data::Match {
                matched: data[1] == 1,
                score: slot(2),
                slot: slot(4),
            },
            Request::MatchResult => return Err(Error::Parameter),
            Request::Slot(wanted) if data[0] <= 1 && slot(1) == wanted => Data::Slot {
                present: data[0] == 1,
                slot: wanted,
            },
            Request::Slot(_) => return Err(Error::Parameter),
            Request::Finger if data[0] <= 1 => Data::Finger(data[0] == 1),
            Request::Finger => return Err(Error::Parameter),
            Request::Count if slot(0) <= MODULE_SLOTS as u16 => Data::Count(slot(0)),
            Request::Count => return Err(Error::Parameter),
            _ => Data::None,
        };
        Ok(Reply { completion, data })
    }
}

pub fn checksum(bytes: &[u8]) -> u8 {
    bytes
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
        .wrapping_neg()
}

pub struct Decoder {
    bytes: [u8; MAX_FRAME],
    used: usize,
    length: usize,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder {
    pub const fn new() -> Self {
        Self {
            bytes: [0; MAX_FRAME],
            used: 0,
            length: 0,
        }
    }

    pub fn empty(&self) -> bool {
        self.used == 0
    }

    fn reset(&mut self, byte: u8) {
        self.used = usize::from(byte == HEADER[0]);
        self.bytes[0] = byte;
        self.length = 0;
    }

    pub fn feed(&mut self, byte: u8) -> Option<Result<Frame, Error>> {
        if self.used < HEADER.len() && byte != HEADER[self.used] {
            self.reset(byte);
            return Some(Err(Error::Noise));
        }
        self.bytes[self.used] = byte;
        self.used += 1;
        if self.used == LINK_BYTES {
            let application = u16::from_be_bytes([self.bytes[8], self.bytes[9]]) as usize;
            let error = if checksum(&self.bytes[..LINK_BYTES]) != 0 {
                Some(Error::HeaderChecksum)
            } else if !(RESPONSE_BYTES..=MAX_APPLICATION).contains(&application) {
                Some(Error::Length)
            } else {
                None
            };
            if let Some(error) = error {
                self.reset(byte);
                return Some(Err(error));
            }
            self.length = LINK_BYTES + application;
        }
        if self.length == 0 || self.used != self.length {
            return None;
        }
        let app = &self.bytes[LINK_BYTES..self.length];
        let mut frame = Frame {
            bytes: [0; MAX_APPLICATION],
            len: app.len(),
        };
        let valid = checksum(app) == 0;
        frame.bytes[..app.len()].copy_from_slice(app);
        self.used = 0;
        self.length = 0;
        Some(if valid {
            Ok(frame)
        } else {
            Err(Error::ApplicationChecksum)
        })
    }
}

#[cfg(test)]
#[path = "fingerprint_protocol_tests.rs"]
mod tests;

#[cfg(kani)]
#[path = "fingerprint_protocol_kani.rs"]
mod proofs;
