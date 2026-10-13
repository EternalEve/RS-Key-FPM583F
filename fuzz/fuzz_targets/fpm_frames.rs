// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

#![no_main]

use libfuzzer_sys::fuzz_target;
use rsk_device::fingerprint_protocol::{Decoder, Request};

fuzz_target!(|data: &[u8]| {
    let mut decoder = Decoder::new();
    for byte in data {
        if let Some(Ok(frame)) = decoder.feed(*byte) {
            for request in [
                Request::MatchResult,
                Request::CaptureResult,
                Request::SaveResult,
                Request::Slot(99),
                Request::Finger,
                Request::Count,
                Request::DeleteResult,
                Request::Sleep,
            ] {
                let _ = frame.decode(request, [0; 4]);
            }
        }
    }
});
