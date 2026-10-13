// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;

// Independent full vectors: manual p16, p22 and p26, including both checksums.
const CAPTURE: [u8; 19] = [
    0xf1, 0x1f, 0xe2, 0x2e, 0xb6, 0x6b, 0xa8, 0x8a, 0, 8, 0x85, 0, 0, 0, 0, 1, 0x11, 1, 0xed,
];
const MATCH: [u8; 28] = [
    0xf1, 0x1f, 0xe2, 0x2e, 0xb6, 0x6b, 0xa8, 0x8a, 0, 0x11, 0x7c, 0, 0, 0, 0, 1, 0x22, 0, 0, 0, 0,
    0, 1, 0x27, 0x0f, 0, 3, 0xa3,
];
const BUSY: [u8; 22] = [
    0xf1, 0x1f, 0xe2, 0x2e, 0xb6, 0x6b, 0xa8, 0x8a, 0, 0x0b, 0x82, 0, 0, 0, 0, 1, 0x22, 0, 0, 0, 4,
    0xd9,
];
const SLOT: [u8; 25] = [
    0xf1, 0x1f, 0xe2, 0x2e, 0xb6, 0x6b, 0xa8, 0x8a, 0, 0x0e, 0x7f, 0, 0, 0, 0, 1, 0x33, 0, 0, 0, 0,
    1, 0, 1, 0xca,
];

fn frame(bytes: &[u8]) -> Result<Frame, Error> {
    let mut decoder = Decoder::new();
    let mut result = None;
    for byte in bytes {
        if let Some(next) = decoder.feed(*byte) {
            result = Some(next);
        }
    }
    assert!(decoder.empty());
    result.expect("a full vector must produce a frame")
}

#[test]
fn manual_vectors_are_independent_of_the_encoder() {
    let mut bytes = [0; MAX_FRAME];
    let n = Request::Capture(1).encode([0; 4], &mut bytes).unwrap();
    assert_eq!(&bytes[..n], &CAPTURE);
    let reply = frame(&MATCH)
        .unwrap()
        .decode(Request::MatchResult, [0; 4])
        .unwrap();
    assert_eq!(
        reply,
        Reply {
            completion: Completion::OK,
            data: Data::Match {
                matched: true,
                score: 9999,
                slot: 3
            },
        }
    );
    assert_eq!(
        frame(&BUSY)
            .unwrap()
            .decode(Request::MatchResult, [0; 4])
            .unwrap(),
        Reply {
            completion: Completion::BUSY,
            data: Data::None,
        }
    );
    assert_eq!(
        frame(&SLOT)
            .unwrap()
            .decode(Request::Slot(1), [0; 4])
            .unwrap()
            .data,
        Data::Slot {
            present: true,
            slot: 1
        }
    );
}

#[test]
fn ordinary_sleep_uses_the_manual_mode_zero_and_strict_ack() {
    let request = [
        0xf1, 0x1f, 0xe2, 0x2e, 0xb6, 0x6b, 0xa8, 0x8a, 0, 8, 0x85, 0, 0, 0, 0, 2, 0x0c, 0, 0xf2,
    ];
    let response = [
        0xf1, 0x1f, 0xe2, 0x2e, 0xb6, 0x6b, 0xa8, 0x8a, 0, 0x0b, 0x82, 0, 0, 0, 0, 2, 0x0c, 0, 0,
        0, 0, 0xf2,
    ];
    let mut encoded = [0; MAX_FRAME];
    let n = Request::Sleep.encode([0; 4], &mut encoded).unwrap();
    assert_eq!(&encoded[..n], &request, "manual p34 ordinary sleep vector");
    assert_eq!(
        frame(&response).unwrap().decode(Request::Sleep, [0; 4]),
        Ok(Reply {
            completion: Completion::OK,
            data: Data::None
        })
    );
    assert_eq!(
        frame(&response).unwrap().decode(Request::Cancel, [0; 4]),
        Err(Error::Command)
    );
    let mut corrupt = response;
    corrupt[21] ^= 1;
    assert!(frame(&corrupt).is_err());
}

#[test]
fn every_split_and_back_to_back_frames_preserve_boundaries() {
    for split in 0..=MATCH.len() {
        let mut decoder = Decoder::new();
        let mut frames = Vec::new();
        for chunk in [&MATCH[..split], &MATCH[split..], &BUSY] {
            for byte in chunk {
                if let Some(next) = decoder.feed(*byte) {
                    frames.push(next.unwrap().decode(Request::MatchResult, [0; 4]).unwrap());
                }
            }
        }
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].completion, Completion::OK);
        assert_eq!(frames[1].completion, Completion::BUSY);
        assert!(decoder.empty());
    }
}

#[test]
fn short_success_unknown_code_wrong_command_and_slot_are_refused() {
    let mut short = BUSY;
    short[20] = 0;
    short[21] = 0xdd;
    assert_eq!(
        frame(&short).unwrap().decode(Request::MatchResult, [0; 4]),
        Err(Error::Length)
    );
    let mut unknown = BUSY;
    unknown[20] = 0x20;
    unknown[21] = 0xbd;
    assert_eq!(
        frame(&unknown)
            .unwrap()
            .decode(Request::MatchResult, [0; 4]),
        Err(Error::Completion)
    );
    assert_eq!(
        frame(&SLOT).unwrap().decode(Request::Slot(2), [0; 4]),
        Err(Error::Parameter)
    );
    let mut stale = MATCH;
    stale[16] = 0x23;
    stale[27] = checksum(&stale[11..27]);
    assert_eq!(
        frame(&stale).unwrap().decode(Request::MatchResult, [0; 4]),
        Err(Error::Command),
        "valid frame for another command must be refused"
    );
    assert_eq!(
        frame(&MATCH)
            .unwrap()
            .decode(Request::CaptureResult, [0; 4]),
        Err(Error::Command)
    );
    assert_eq!(
        frame(&MATCH).unwrap().decode(Request::MatchResult, [1; 4]),
        Err(Error::Password)
    );
}

#[test]
fn both_checksums_and_all_header_lengths_are_checked_before_indexing() {
    let mut bad = MATCH;
    bad[10] ^= 1;
    assert!(matches!(
        frame(&bad),
        Err(Error::HeaderChecksum) | Err(Error::Noise)
    ));
    bad = MATCH;
    bad[27] ^= 1;
    assert_eq!(
        frame(&bad).err(),
        Some(Error::ApplicationChecksum),
        "checksum-corrupt frame must be refused"
    );
    for length in [0u16, 1, 10, 78, 255, 256, 65535] {
        let mut decoder = Decoder::new();
        let mut prefix = [0; LINK_BYTES];
        prefix[..8].copy_from_slice(&HEADER);
        prefix[8..10].copy_from_slice(&length.to_be_bytes());
        prefix[10] = checksum(&prefix[..10]);
        let mut error = None;
        for byte in prefix {
            if let Some(result) = decoder.feed(byte) {
                error = result.err();
            }
        }
        assert_eq!(error, Some(Error::Length));
        let mut good = None;
        for byte in MATCH {
            if let Some(Ok(frame)) = decoder.feed(byte) {
                good = Some(frame);
            }
        }
        assert_eq!(
            good.unwrap()
                .decode(Request::MatchResult, [0; 4])
                .unwrap()
                .completion,
            Completion::OK
        );
    }
}

#[test]
fn a_partial_frame_is_pending_and_payload_55_is_data() {
    let mut decoder = Decoder::new();
    for byte in &MATCH[..MATCH.len() - 1] {
        assert!(decoder.feed(*byte).is_none());
    }
    assert!(!decoder.empty());
    assert!(decoder.feed(MATCH[MATCH.len() - 1]).unwrap().is_ok());
    let mut payload = MATCH;
    payload[23] = 0x55;
    payload[27] = checksum(&payload[11..27]);
    assert!(
        frame(&payload)
            .unwrap()
            .decode(Request::MatchResult, [0; 4])
            .is_ok()
    );
}

#[test]
fn noise_and_oversize_streams_recover_without_accepting_bad_frames() {
    let mut decoder = Decoder::new();
    for byte in [0, 0x55, 0xff, 0xf1, 0xf1] {
        assert!(decoder.feed(byte).is_none_or(|r| r.is_err()));
    }
    let mut accepted = 0;
    for byte in MATCH.into_iter().chain(SLOT) {
        if let Some(Ok(_)) = decoder.feed(byte) {
            accepted += 1;
        }
    }
    assert_eq!(accepted, 2);
}

#[test]
fn encoder_rejects_out_of_range_targets_and_sample_numbers() {
    for request in [
        Request::Capture(0),
        Request::Capture(7),
        Request::Save(100),
        Request::Slot(100),
        Request::Delete(u16::MAX),
    ] {
        assert_eq!(
            request.encode([0; 4], &mut [0; MAX_FRAME]),
            Err(Error::Parameter)
        );
    }
    assert_eq!(
        Request::Match.encode([0; 4], &mut [0; 17]),
        Err(Error::Length)
    );
    for request in [
        Request::Clear,
        Request::Delete(99),
        Request::Save(99),
        Request::Count,
        Request::Cancel,
        Request::Finger,
        Request::SaveResult,
        Request::DeleteResult,
    ] {
        let mut bytes = [0; MAX_FRAME];
        let n = request.encode([0; 4], &mut bytes).unwrap();
        assert_eq!(checksum(&bytes[..LINK_BYTES]), 0);
        assert_eq!(checksum(&bytes[LINK_BYTES..n]), 0);
    }
}
