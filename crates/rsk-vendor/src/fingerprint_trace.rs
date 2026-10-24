// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Bounded volatile observations for an explicitly enabled touch measurement image.

pub const VERSION: u8 = 1;
pub const CAPACITY: usize = 128;
pub const RECORD_LEN: usize = 16;
pub const HEADER_LEN: usize = 20;
pub const RECORDS_PER_PAGE: usize = 2;
pub const PAGE_LEN: usize = HEADER_LEN + RECORDS_PER_PAGE * RECORD_LEN;
pub const PAGES: usize = CAPACITY / RECORDS_PER_PAGE;
const _: () = assert!(
    PAGE_LEN + 2 <= 57,
    "touch trace page must fit one CTAPHID report"
);

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum Event {
    Initialized = 1,
    Resample = 2,
    Edge = 3,
    Open = 4,
    Close = 5,
    Taken = 6,
    Service = 7,
}

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum Level {
    Low = 0,
    High = 1,
    Unknown = 2,
}

#[derive(Clone, Copy)]
pub struct Snapshot {
    pub phase: u8,
    pub consent: u8,
    pub level: Level,
    pub window: u32,
    pub adapter_stage: u8,
}

pub struct Trace {
    records: [[u8; RECORD_LEN]; CAPACITY],
    head: usize,
    len: usize,
    sequence: u32,
}

impl Default for Trace {
    fn default() -> Self {
        Self::new()
    }
}

impl Trace {
    pub const fn new() -> Self {
        Self {
            records: [[0; RECORD_LEN]; CAPACITY],
            head: 0,
            len: 0,
            sequence: 0,
        }
    }

    pub fn record(&mut self, ms: u32, event: Event, snapshot: Snapshot) {
        let mut record = [0; RECORD_LEN];
        record[..4].copy_from_slice(&self.sequence.to_le_bytes());
        record[4..8].copy_from_slice(&ms.to_le_bytes());
        record[8..12].copy_from_slice(&snapshot.window.to_le_bytes());
        record[12..].copy_from_slice(&[
            event as u8,
            snapshot.phase,
            snapshot.level as u8,
            snapshot.consent,
        ]);
        self.records[self.head] = record;
        self.head = (self.head + 1) % CAPACITY;
        self.len = (self.len + 1).min(CAPACITY);
        self.sequence = self.sequence.wrapping_add(1);
    }

    pub fn page(&self, page: u8, live: Snapshot) -> Option<[u8; PAGE_LEN]> {
        if page as usize >= PAGES {
            return None;
        }
        let start = page as usize * RECORDS_PER_PAGE;
        let count = self.len.saturating_sub(start).min(RECORDS_PER_PAGE);
        let mut out = [0; PAGE_LEN];
        out[0] = VERSION;
        out[1] = count as u8;
        out[2..4].copy_from_slice(&(self.len as u16).to_le_bytes());
        out[4..8].copy_from_slice(&self.sequence.wrapping_sub(self.len as u32).to_le_bytes());
        out[8..12].copy_from_slice(&self.sequence.to_le_bytes());
        out[12..16].copy_from_slice(&[
            live.level as u8,
            live.phase,
            live.consent,
            live.adapter_stage,
        ]);
        out[16..20].copy_from_slice(&live.window.to_le_bytes());
        let oldest = if self.len == CAPACITY { self.head } else { 0 };
        for index in 0..count {
            let at = HEADER_LEN + index * RECORD_LEN;
            out[at..at + RECORD_LEN]
                .copy_from_slice(&self.records[(oldest + start + index) % CAPACITY]);
        }
        Some(out)
    }
}

#[cfg(test)]
#[path = "fingerprint_trace_tests.rs"]
mod tests;
