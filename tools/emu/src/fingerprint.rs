// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Scripted module results below the real FIDO policy and persistent store.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Poll;
use std::time::{Duration, Instant};

use rsk_fido::CtapError;
use rsk_fido::fingerprint::{
    self, Attempt, Bindings, Failure, Lifecycle, Match, Matching, Observation, Operations, Sensor,
};
use rsk_fs::Fs;

use crate::signals::Signals;
use crate::store::EmuStore;

const SLOT: u16 = 7;

#[derive(Clone, Copy)]
enum ResultKind {
    Match,
    UnknownSlot,
    NoMatch,
    Stale,
    Communication,
    InputTimeout,
    UnknownTimeout,
    Wait,
    GenerationChange,
}

struct Event {
    result: ResultKind,
    input_ms: u64,
    processing_ms: u64,
    drain_ms: u64,
}

impl Event {
    fn read(path: &PathBuf) -> Result<Self, Failure> {
        let text = std::fs::read_to_string(path).map_err(|_| Failure::Communication)?;
        let mut fields = text.split_whitespace();
        let result = match fields.next() {
            Some("match") => ResultKind::Match,
            Some("unknown") => ResultKind::UnknownSlot,
            Some("nomatch") => ResultKind::NoMatch,
            Some("stale") => ResultKind::Stale,
            Some("communication") => ResultKind::Communication,
            Some("input-timeout") => ResultKind::InputTimeout,
            Some("unknown-timeout") => ResultKind::UnknownTimeout,
            Some("wait") => ResultKind::Wait,
            Some("generation") => ResultKind::GenerationChange,
            _ => return Err(Failure::Communication),
        };
        let mut number = || {
            fields
                .next()
                .and_then(|n| n.parse::<u64>().ok())
                .filter(|n| *n <= fingerprint::VERIFY_TIMEOUT_MS)
                .ok_or(Failure::Communication)
        };
        let input_ms = number()?;
        let processing_ms = number()?;
        let drain_ms = number()?;
        if fields.next().is_some() {
            return Err(Failure::Communication);
        }
        Ok(Self {
            result,
            input_ms,
            processing_ms,
            drain_ms,
        })
    }
}

pub struct Script {
    path: PathBuf,
    attach: Rc<Cell<Instant>>,
    signals: Arc<Signals>,
    generation: Rc<Cell<u32>>,
    operations: Operations,
    event: Option<Event>,
    drain_until_ms: u64,
}

impl Script {
    pub fn reset(&mut self) {
        self.operations = Operations::new();
        self.event = None;
        self.signals.end();
    }
}

impl Sensor for Script {
    fn now_ms(&self) -> u64 {
        self.attach.get().elapsed().as_millis() as u64
    }
    fn ready(&self) -> bool {
        self.operations.state() == Lifecycle::Idle
    }
    fn start(&mut self, channel: u32) -> Result<Attempt, Failure> {
        let event = Event::read(&self.path)?;
        let attempt = self.operations.begin(channel, self.now_ms())?;
        self.event = Some(event);
        self.signals.clear_cancel();
        self.signals.set_cancellable(true);
        self.signals.set_up_pending(true);
        eprintln!("fingerprint: start {} channel {}", attempt.id, channel);
        Ok(attempt)
    }
    fn poll(&mut self) -> Poll<Observation> {
        let Lifecycle::Active(mut attempt) = self.operations.state() else {
            return Poll::Pending;
        };
        let event = self.event.as_ref().expect("active attempt has an event");
        let elapsed = self.now_ms().saturating_sub(attempt.started_ms);
        if elapsed < event.input_ms || matches!(event.result, ResultKind::Wait) {
            return Poll::Pending;
        }
        if !matches!(event.result, ResultKind::InputTimeout) {
            if self.operations.matching() == Matching::NotStarted {
                let stage = if matches!(event.result, ResultKind::UnknownTimeout) {
                    Matching::Unknown
                } else {
                    Matching::Started
                };
                self.operations.mark_matching(attempt, stage);
                self.signals.set_up_pending(false);
                eprintln!("fingerprint: matching {}", attempt.id);
            }
            if elapsed < event.input_ms + event.processing_ms
                || matches!(event.result, ResultKind::UnknownTimeout)
            {
                return Poll::Pending;
            }
        }
        let result = match event.result {
            ResultKind::Match | ResultKind::Stale | ResultKind::GenerationChange => {
                if matches!(event.result, ResultKind::Stale) {
                    attempt.id -= 1;
                }
                if matches!(event.result, ResultKind::GenerationChange) {
                    self.generation.set(self.generation.get().saturating_add(1));
                }
                Ok(Match {
                    slot: SLOT,
                    interaction: true,
                })
            }
            ResultKind::UnknownSlot => Ok(Match {
                slot: SLOT + 1,
                interaction: true,
            }),
            ResultKind::NoMatch => Err(Failure::NoMatch),
            ResultKind::InputTimeout => Err(Failure::InputTimeout),
            _ => Err(Failure::Communication),
        };
        eprintln!("fingerprint: result {}", attempt.id);
        Poll::Ready(Observation { attempt, result })
    }
    fn cancelled(&self) -> bool {
        self.signals.cancelled()
    }
    fn matching(&self) -> Matching {
        self.operations.matching()
    }
    fn wait(&mut self) {
        std::thread::sleep(Duration::from_millis(1));
    }
    fn finish(&mut self, attempt: Attempt) {
        let now = self.now_ms();
        self.operations.finish(attempt, now);
        self.drain_until_ms = now.saturating_add(self.event.as_ref().map_or(0, |e| e.drain_ms));
        self.signals.set_cancellable(false);
        self.signals.set_up_pending(false);
        eprintln!("fingerprint: finish {}", attempt.id);
    }
    fn service_idle(&mut self) {
        let now = self.now_ms();
        self.operations.expire_drain(now);
        if let Lifecycle::Draining { attempt, .. } = self.operations.state()
            && now >= self.drain_until_ms
            && self.operations.drained(attempt)
        {
            self.event = None;
            eprintln!("fingerprint: idle {}", attempt.id);
        }
    }
}

pub struct Directory {
    enabled: bool,
    generation: Rc<Cell<u32>>,
}

impl Bindings<EmuStore> for Directory {
    fn configuration(&mut self, fs: &mut Fs<EmuStore>) -> Result<Option<u32>, CtapError> {
        Ok((self.enabled
            && fs
                .try_has_data(rsk_fido::consts::EF_PIN)
                .map_err(|_| CtapError::Other)?)
        .then(|| self.generation.get()))
    }
    fn authorizes(
        &mut self,
        fs: &mut Fs<EmuStore>,
        slot: u16,
        generation: u32,
    ) -> Result<bool, CtapError> {
        Ok(slot == SLOT && self.configuration(fs)? == Some(generation))
    }
}

pub fn fixture(
    path: PathBuf,
    initialize: bool,
    attach: Rc<Cell<Instant>>,
    signals: Arc<Signals>,
    fs: &mut Fs<EmuStore>,
) -> Result<(RefCell<Script>, RefCell<Directory>), CtapError> {
    Event::read(&path).map_err(|_| CtapError::Other)?;
    let generation = Rc::new(Cell::new(1));
    let mut directory = Directory {
        enabled: false,
        generation: generation.clone(),
    };
    if initialize {
        fingerprint::initialize_unconfigured(fs, &mut directory)?;
    }
    directory.enabled = true;
    Ok((
        RefCell::new(Script {
            path,
            attach,
            signals,
            generation,
            operations: Operations::new(),
            event: None,
            drain_until_ms: 0,
        }),
        RefCell::new(directory),
    ))
}
