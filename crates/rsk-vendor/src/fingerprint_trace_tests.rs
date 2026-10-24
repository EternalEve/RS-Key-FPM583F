// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;

fn snapshot() -> Snapshot {
    Snapshot {
        phase: 1,
        consent: 2,
        level: Level::Low,
        window: 8,
        adapter_stage: 0,
    }
}

#[test]
fn empty_and_repeated_reads_do_not_append_drain_or_change_the_trace() {
    let mut trace = Trace::new();
    let empty = trace.page(0, snapshot()).unwrap();
    assert_eq!(&empty[..12], &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(&empty[HEADER_LEN..], &[0; PAGE_LEN - HEADER_LEN]);
    trace.record(42, Event::Open, snapshot());
    let first = trace.page(0, snapshot()).unwrap();
    assert_eq!(trace.page(0, snapshot()), Some(first));
    assert_eq!(trace.len, 1);
    assert_eq!(trace.sequence, 1);
    assert_eq!(
        &first[HEADER_LEN..HEADER_LEN + RECORD_LEN],
        &[0, 0, 0, 0, 42, 0, 0, 0, 8, 0, 0, 0, 4, 1, 0, 2]
    );
}

#[test]
fn overwritten_records_are_returned_oldest_first_without_duplicating_pages() {
    let mut trace = Trace::new();
    for seq in 0..CAPACITY as u32 + 5 {
        trace.record(seq * 10, Event::Edge, snapshot());
    }
    for page in 0..PAGES {
        let body = trace.page(page as u8, snapshot()).unwrap();
        assert_eq!(&body[..4], &[1, 2, CAPACITY as u8, 0]);
        assert_eq!(u32::from_le_bytes(body[4..8].try_into().unwrap()), 5);
        assert_eq!(
            u32::from_le_bytes(body[8..12].try_into().unwrap()),
            CAPACITY as u32 + 5
        );
        for index in 0..RECORDS_PER_PAGE {
            let at = HEADER_LEN + index * RECORD_LEN;
            let expected = 5 + (page * RECORDS_PER_PAGE + index) as u32;
            assert_eq!(
                u32::from_le_bytes(body[at..at + 4].try_into().unwrap()),
                expected
            );
            assert_eq!(
                u32::from_le_bytes(body[at + 4..at + 8].try_into().unwrap()),
                expected * 10
            );
        }
    }
    assert!(trace.page(PAGES as u8, snapshot()).is_none());
    assert!(trace.page(u8::MAX, snapshot()).is_none());
}

#[test]
fn partial_pages_and_sequence_wrap_preserve_their_bounds() {
    let mut trace = Trace::new();
    trace.sequence = u32::MAX;
    for ms in 0..3 {
        trace.record(ms, Event::Resample, snapshot());
    }
    let body = trace.page(1, snapshot()).unwrap();
    assert_eq!(&body[..4], &[1, 1, 3, 0]);
    assert_eq!(u32::from_le_bytes(body[4..8].try_into().unwrap()), u32::MAX);
    assert_eq!(u32::from_le_bytes(body[8..12].try_into().unwrap()), 2);
    assert_eq!(u32::from_le_bytes(body[20..24].try_into().unwrap()), 1);
    assert!(body[HEADER_LEN + RECORD_LEN..].iter().all(|&b| b == 0));
    assert!(
        trace.page(2, snapshot()).unwrap()[HEADER_LEN..]
            .iter()
            .all(|&b| b == 0)
    );
    assert!(core::mem::size_of::<Trace>() <= CAPACITY * RECORD_LEN + 32);
}
