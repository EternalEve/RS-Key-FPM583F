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
use rsk_fido::bio_sensor::{self, Capabilities, Command, Feedback, Inventory};
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
    PoorQuality,
    NoTransition,
    NoActivity,
    SaveFailure,
    SaveUnknown,
    DeleteFailure,
    ClearFailure,
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
            Some("poor-quality") => ResultKind::PoorQuality,
            Some("no-transition") => ResultKind::NoTransition,
            Some("no-activity") => ResultKind::NoActivity,
            Some("save-failure") => ResultKind::SaveFailure,
            Some("save-unknown") => ResultKind::SaveUnknown,
            Some("delete-failure") => ResultKind::DeleteFailure,
            Some("clear-failure") => ResultKind::ClearFailure,
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
    wire: Option<crate::fingerprint_wire::Fixture>,
    path: PathBuf,
    attach: Rc<Cell<Instant>>,
    signals: Arc<Signals>,
    generation: Rc<Cell<u32>>,
    operations: Operations,
    event: Option<Event>,
    drain_until_ms: u64,
    inventory_path: Option<PathBuf>,
    command: Option<Command>,
}

impl Script {
    pub fn reset(&mut self) -> Result<(), CtapError> {
        if let Some(wire) = self.wire.as_mut() {
            return wire.reset();
        }
        self.operations = Operations::new();
        self.event = None;
        self.command = None;
        self.signals.end();
        Ok(())
    }
}

impl Sensor for Script {
    fn prepare(&mut self, deadline_ms: u64) -> Result<(), CtapError> {
        if let Some(wire) = self.wire.as_mut() {
            return wire.driver.prepare(deadline_ms);
        }
        self.service_idle();
        if self.cancelled() {
            Err(CtapError::KeepAliveCancel)
        } else if self.ready() {
            Ok(())
        } else {
            Err(CtapError::Other)
        }
    }
    fn release(&mut self, enrollment: bool) {
        if let Some(wire) = self.wire.as_mut() {
            wire.driver.release(enrollment);
        }
    }
    fn deadline_ms(&self, deadline_ms: u64) -> u64 {
        self.wire
            .as_ref()
            .map_or(deadline_ms, |wire| wire.driver.deadline_ms(deadline_ms))
    }
    fn now_ms(&self) -> u64 {
        if let Some(wire) = &self.wire {
            return wire.driver.now_ms();
        }
        self.attach.get().elapsed().as_millis() as u64
    }
    fn ready(&self) -> bool {
        if let Some(wire) = &self.wire {
            return wire.driver.ready();
        }
        self.operations.state() == Lifecycle::Idle
    }
    fn start(&mut self, channel: u32) -> Result<Attempt, Failure> {
        if let Some(wire) = self.wire.as_mut() {
            return wire.driver.start(channel);
        }
        let event = Event::read(&self.path)?;
        let attempt = self.operations.begin(channel, self.now_ms())?;
        self.event = Some(event);
        self.command = None;
        self.signals.set_cancellable(true);
        self.signals.set_up_pending(true);
        eprintln!("fingerprint: start {} channel {}", attempt.id, channel);
        Ok(attempt)
    }
    fn poll(&mut self) -> Poll<Observation> {
        if let Some(wire) = self.wire.as_mut() {
            return wire.driver.poll();
        }
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
                    slot: if self.inventory_path.is_some() {
                        match self.read_inventory() {
                            Ok(inventory) => (0..bio_sensor::MODULE_SLOTS as u16)
                                .find(|s| inventory.contains(*s))
                                .unwrap_or(u16::MAX),
                            Err(failure) => {
                                return Poll::Ready(Observation {
                                    attempt,
                                    result: Err(failure),
                                });
                            }
                        }
                    } else {
                        SLOT
                    },
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
        if let Some(wire) = &self.wire {
            return wire.driver.cancelled();
        }
        self.signals.cancelled()
    }
    fn matching(&self) -> Matching {
        if let Some(wire) = &self.wire {
            return wire.driver.matching();
        }
        self.operations.matching()
    }
    fn wait(&mut self) {
        if let Some(wire) = self.wire.as_mut() {
            wire.driver.wait();
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    fn finish(&mut self, attempt: Attempt) {
        if let Some(wire) = self.wire.as_mut() {
            wire.driver.finish(attempt);
            return;
        }
        let now = self.now_ms();
        self.operations.finish(attempt, now);
        self.drain_until_ms = now.saturating_add(if self.command == Some(Command::Inventory) {
            0
        } else {
            self.event.as_ref().map_or(0, |e| e.drain_ms)
        });
        self.signals.set_cancellable(self.command.is_some());
        self.signals.set_up_pending(false);
        eprintln!("fingerprint: finish {}", attempt.id);
    }
    fn service_idle(&mut self) {
        if let Some(wire) = self.wire.as_mut() {
            wire.driver.service_idle();
            return;
        }
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
    fn capabilities(&self) -> Option<Capabilities> {
        if let Some(wire) = &self.wire {
            return wire.driver.capabilities();
        }
        self.inventory_path.as_ref().map(|_| Capabilities {
            samples: 3,
            slots: bio_sensor::MODULE_SLOTS as u16,
        })
    }
    fn start_management(&mut self, channel: u32, command: Command) -> Result<Attempt, Failure> {
        if let Some(wire) = self.wire.as_mut() {
            return wire.driver.start_management(channel, command);
        }
        if self.inventory_path.is_none() {
            return Err(Failure::Unavailable);
        }
        let attempt = self.start(channel)?;
        self.command = Some(command);
        if command == Command::Inventory {
            self.signals.set_up_pending(false);
        }
        eprintln!("fingerprint: management {command:?}");
        Ok(attempt)
    }
    fn poll_management(&mut self) -> Poll<bio_sensor::Observation> {
        if let Some(wire) = self.wire.as_mut() {
            return wire.driver.poll_management();
        }
        let Lifecycle::Active(mut attempt) = self.operations.state() else {
            return Poll::Pending;
        };
        if self.command == Some(Command::Inventory) {
            return Poll::Ready(bio_sensor::Observation {
                attempt,
                result: self.read_inventory().map(bio_sensor::Result::Inventory),
            });
        }
        let event = self.event.as_ref().expect("active attempt has event");
        let elapsed = self.now_ms().saturating_sub(attempt.started_ms);
        if elapsed < event.input_ms || matches!(event.result, ResultKind::Wait) {
            return Poll::Pending;
        }
        self.signals.set_up_pending(false);
        if elapsed < event.input_ms + event.processing_ms {
            return Poll::Pending;
        }
        if matches!(event.result, ResultKind::Stale) {
            attempt.id -= 1;
        }
        let feedback = match event.result {
            ResultKind::PoorQuality => Feedback::PoorQuality,
            ResultKind::NoTransition => Feedback::NoTransition,
            ResultKind::NoActivity => Feedback::NoActivity,
            _ => Feedback::Good,
        };
        let failure = matches!(
            event.result,
            ResultKind::Communication
                | ResultKind::SaveFailure
                | ResultKind::DeleteFailure
                | ResultKind::ClearFailure
        );
        let unknown = matches!(event.result, ResultKind::SaveUnknown);
        let result = if failure {
            Err(Failure::Communication)
        } else {
            self.apply_management(feedback, unknown)
        };
        Poll::Ready(bio_sensor::Observation { attempt, result })
    }
}

impl Script {
    fn read_inventory(&mut self) -> Result<Inventory, Failure> {
        let path = self.inventory_path.as_ref().ok_or(Failure::Unavailable)?;
        let bytes = std::fs::read(path).map_err(|_| Failure::Communication)?;
        Ok(Inventory(
            bytes.try_into().map_err(|_| Failure::Communication)?,
        ))
    }
    fn persist_inventory(&self, inventory: Inventory) -> Result<(), Failure> {
        use std::io::Write;
        let path = self.inventory_path.as_ref().ok_or(Failure::Unavailable)?;
        let temporary = path.with_extension("inventory.tmp");
        let mut file = std::fs::File::create(&temporary).map_err(|_| Failure::Communication)?;
        file.write_all(&inventory.0)
            .and_then(|_| file.sync_all())
            .map_err(|_| Failure::Communication)?;
        std::fs::rename(&temporary, path).map_err(|_| Failure::Communication)
    }
    fn apply_management(
        &mut self,
        feedback: Feedback,
        unknown: bool,
    ) -> Result<bio_sensor::Result, Failure> {
        let command = self.command.ok_or(Failure::Unavailable)?;
        let mut inventory = if command == Command::Clear {
            Inventory::default()
        } else {
            self.read_inventory()?
        };
        let result = match command {
            Command::Capture {
                slot, remaining, ..
            } => {
                let remaining = if feedback == Feedback::Good {
                    remaining.saturating_sub(1)
                } else {
                    remaining
                };
                let saved = remaining == 0;
                if saved {
                    inventory.set(slot, true);
                    self.persist_inventory(inventory)?;
                }
                bio_sensor::Result::Sample {
                    feedback,
                    remaining,
                    saved,
                }
            }
            Command::Delete { slot } => {
                inventory.set(slot, false);
                self.persist_inventory(inventory)?;
                bio_sensor::Result::Deleted
            }
            Command::Inventory => bio_sensor::Result::Inventory(inventory),
            Command::Clear => {
                self.persist_inventory(Inventory::default())?;
                bio_sensor::Result::Cleared
            }
        };
        if unknown {
            Err(Failure::Communication)
        } else {
            Ok(result)
        }
    }
}

pub struct Directory {
    enabled: bool,
    generation: Rc<Cell<u32>>,
    persistent: Option<rsk_fido::bio_directory::Directory>,
}

impl Bindings<EmuStore> for Directory {
    fn management(&self) -> bool {
        self.persistent.is_some()
    }
    fn configuration(&mut self, fs: &mut Fs<EmuStore>) -> Result<Option<u32>, CtapError> {
        if let Some(directory) = self.persistent.as_mut() {
            return directory.configuration(fs);
        }
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
        if let Some(directory) = self.persistent.as_mut() {
            return directory.authorizes(fs, slot, generation);
        }
        Ok(slot == SLOT && self.configuration(fs)? == Some(generation))
    }
}

#[allow(clippy::too_many_arguments)] // One-time wiring of the isolated module fixture.
pub fn fixture(
    path: PathBuf,
    initialize: bool,
    management: bool,
    protocol: bool,
    power: bool,
    attach: Rc<Cell<Instant>>,
    signals: Arc<Signals>,
    fs: &mut Fs<EmuStore>,
) -> Result<(RefCell<Script>, RefCell<Directory>), CtapError> {
    if !protocol {
        Event::read(&path).map_err(|_| CtapError::Other)?;
    }
    let generation = Rc::new(Cell::new(1));
    let mut directory = Directory {
        enabled: false,
        generation: generation.clone(),
        persistent: management.then_some(rsk_fido::bio_directory::Directory),
    };
    if initialize {
        fingerprint::initialize_unconfigured(fs, &mut directory)?;
    }
    directory.enabled = true;
    let inventory_path = management.then(|| path.with_extension("inventory"));
    if let Some(inventory_path) = &inventory_path {
        let recovered = (|| -> Result<(), CtapError> {
            if !inventory_path.exists() && rsk_fido::bio_directory::read(fs)?.head.is_none() {
                std::fs::write(inventory_path, Inventory::default().0)
                    .map_err(|_| CtapError::Other)?;
            }
            let inventory = std::fs::read(inventory_path).map_err(|_| CtapError::Other)?;
            let inventory = Inventory(inventory.try_into().map_err(|_| CtapError::Other)?);
            if !protocol && !rsk_fido::bio_directory::reset_pending(fs)? {
                rsk_fido::bio_directory::reconcile(fs, inventory, None)?;
            }
            Ok(())
        })();
        if let Err(error) = recovered {
            eprintln!("emu: biometric state isolated ({error:?}); explicit CTAP Reset required");
        }
    }
    let wire = if protocol {
        Some(crate::fingerprint_wire::Fixture::new(
            path.clone(),
            attach.clone(),
            signals.clone(),
            power,
        )?)
    } else {
        None
    };
    Ok((
        RefCell::new(Script {
            wire,
            path,
            attach,
            signals,
            generation,
            operations: Operations::new(),
            event: None,
            drain_until_ms: 0,
            inventory_path,
            command: None,
        }),
        RefCell::new(directory),
    ))
}
