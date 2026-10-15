// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Serialized module facts, shared by the physical UART and the byte-level fixture.

use core::task::Poll;

use rsk_fido::bio_sensor::{self, Capabilities, Command, Feedback, Inventory, MODULE_SLOTS};
use rsk_fido::fingerprint::{
    Attempt, Failure, Lifecycle, Match, Matching, Observation, Operations, Sensor,
};

use crate::fingerprint_exchange::Exchange;
pub use crate::fingerprint_exchange::{Failure as WireFailure, Progress};
use crate::fingerprint_io::{STEP_BYTES, Stream};
pub use crate::fingerprint_power::BOOT_TIMEOUT_MS;
use crate::fingerprint_power::{Phase, Power};
use crate::fingerprint_protocol::{Completion, Data, ENROLL_SAMPLES, Error, Reply, Request};

pub const BOOT_DELAY_MS: u64 = 200;
pub const QUERY_INTERVAL_MS: u64 = 200;

pub trait Runtime {
    fn now_ms(&self) -> u64;
    fn cancelled(&self) -> bool;
    fn wait(&mut self);
    fn processing(&mut self) {}
    fn waiting(&mut self, _waiting: bool) {}
    fn end(&mut self) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub request: Request,
    pub progress: Progress,
    pub failure: WireFailure,
}

#[derive(Clone, Copy)]
enum Purpose {
    Boot,
    Sleep,
    Verify,
    Management(Command),
}

#[derive(Clone, Copy)]
enum Step {
    Boot,
    Sleep,
    Match,
    MatchResult,
    Finger,
    Capture(u8),
    CaptureResult,
    Save,
    SaveResult,
    Delete,
    DeleteResult,
    CountBefore,
    Slot(u16),
    CountAfter,
    Cancel,
    CancelledResult(Request),
}

#[derive(Clone, Copy)]
struct Temporary {
    slot: u16,
    generation: u32,
    next: u8,
}

#[derive(Clone, Copy)]
enum Outcome {
    Verify(Result<Match, Failure>),
    Management(Result<bio_sensor::Result, Failure>),
}

pub struct Driver<U, C> {
    stream: U,
    runtime: C,
    operations: Operations,
    purpose: Purpose,
    step: Step,
    exchange: Option<Exchange>,
    due_ms: u64,
    power: Power,
    request_deadline_ms: Option<u64>,
    healthy: bool,
    temporary: Option<Temporary>,
    inventory: Inventory,
    inventory_count: u16,
    outcome: Option<Outcome>,
    terminal: bool,
    diagnostic: Option<Diagnostic>,
    last_progress: Progress,
}

impl<U: Stream, C: Runtime> Driver<U, C> {
    pub fn new(stream: U, runtime: C) -> Self {
        let now_ms = runtime.now_ms();
        let power = Power::new(stream.power_controlled(), now_ms);
        Self {
            stream,
            runtime,
            operations: Operations::new(),
            purpose: Purpose::Boot,
            step: Step::Boot,
            exchange: None,
            due_ms: now_ms.saturating_add(BOOT_DELAY_MS),
            power,
            request_deadline_ms: None,
            healthy: true,
            temporary: None,
            inventory: Inventory::default(),
            inventory_count: 0,
            outcome: None,
            terminal: false,
            diagnostic: None,
            last_progress: Progress {
                sent: 0,
                wire_complete: false,
                response_complete: false,
            },
        }
    }

    pub fn diagnostic(&self) -> Option<Diagnostic> {
        self.diagnostic
    }

    pub fn power_phase(&self) -> Phase {
        self.power.phase()
    }

    fn wire_request(&self) -> Request {
        match self.step {
            Step::Boot | Step::CountBefore | Step::CountAfter => Request::Count,
            Step::Sleep => Request::Sleep,
            Step::Match => Request::Match,
            Step::MatchResult => Request::MatchResult,
            Step::Finger => Request::Finger,
            Step::Capture(n) => Request::Capture(n),
            Step::CaptureResult => Request::CaptureResult,
            Step::Save => Request::Save(self.temporary.map_or(u16::MAX, |t| t.slot)),
            Step::SaveResult => Request::SaveResult,
            Step::Delete => match self.purpose {
                Purpose::Management(Command::Delete { slot }) => Request::Delete(slot),
                _ => Request::Clear,
            },
            Step::DeleteResult => Request::DeleteResult,
            Step::Slot(slot) => Request::Slot(slot),
            Step::Cancel => Request::Cancel,
            Step::CancelledResult(request) => request,
        }
    }

    fn schedule(&mut self, step: Step, delay: u64) {
        self.runtime.waiting(matches!(step, Step::Finger));
        self.step = step;
        self.exchange = None;
        self.due_ms = self.runtime.now_ms().saturating_add(delay);
    }

    fn fail(&mut self, failure: Failure, terminal: bool) {
        self.terminal = terminal;
        self.healthy &= terminal;
        if !self.healthy {
            self.power.fail();
        }
        self.temporary = None;
        self.exchange = None;
        self.outcome = match self.purpose {
            Purpose::Verify => Some(Outcome::Verify(Err(failure))),
            Purpose::Management(_) => Some(Outcome::Management(Err(failure))),
            Purpose::Boot | Purpose::Sleep => None,
        };
        if matches!(self.operations.state(), Lifecycle::Draining { .. }) && terminal {
            self.complete_drain();
        }
    }

    fn wire_fault(&mut self, failure: WireFailure) {
        self.diagnostic = Some(Diagnostic {
            request: self.wire_request(),
            progress: self
                .exchange
                .as_ref()
                .map_or(self.last_progress, |e| e.progress),
            failure,
        });
        self.fail(Failure::Communication, false);
    }

    fn complete_drain(&mut self) {
        if let Lifecycle::Draining { attempt, .. } = self.operations.state() {
            self.operations.drained(attempt);
        }
        self.exchange = None;
        self.outcome = None;
        self.terminal = false;
        self.runtime.end();
    }

    fn pump(&mut self) {
        if !self.healthy || self.terminal {
            return;
        }
        let now_ms = self.runtime.now_ms();
        if let Lifecycle::Active(attempt) = self.operations.state()
            && (now_ms < attempt.started_ms || now_ms >= self.deadline_ms(attempt.deadline_ms))
        {
            self.fail(Failure::Timeout, false);
            return;
        }
        self.operations.expire_drain(now_ms);
        if self.operations.state() == Lifecycle::Unavailable
            || (matches!(self.power.phase(), Phase::Starting | Phase::Stopping)
                && now_ms >= self.power.deadline_ms)
        {
            self.fail(Failure::Unavailable, false);
            return;
        }
        if now_ms < self.due_ms {
            return;
        }
        if self.exchange.is_none() {
            self.last_progress = Progress {
                sent: 0,
                wire_complete: false,
                response_complete: false,
            };
            match Exchange::new(self.wire_request(), now_ms) {
                Ok(exchange) => self.exchange = Some(exchange),
                Err(error) => {
                    self.wire_fault(WireFailure::Protocol(error));
                    return;
                }
            }
        }
        let exchange = self.exchange.as_mut().expect("created above");
        let result = exchange.poll(&mut self.stream, now_ms);
        self.last_progress = exchange.progress;
        if matches!(self.purpose, Purpose::Verify)
            && matches!(self.step, Step::Match | Step::MatchResult)
            && exchange.progress.sent > 0
            && let Lifecycle::Active(attempt) = self.operations.state()
        {
            self.operations.mark_matching(attempt, Matching::Unknown);
        }
        match result {
            Poll::Pending => {}
            Poll::Ready(Err(failure)) => self.wire_fault(failure),
            Poll::Ready(Ok(frame)) => match frame.decode(self.wire_request(), [0; 4]) {
                Err(error) => self.wire_fault(WireFailure::Protocol(error)),
                Ok(reply) => {
                    self.exchange = None;
                    if matches!(self.operations.state(), Lifecycle::Draining { .. }) {
                        self.drain_reply(reply);
                    } else {
                        self.reply(reply);
                    }
                }
            },
        }
    }

    fn reply(&mut self, reply: Reply) {
        if reply.completion == Completion::BUSY {
            if matches!(
                self.step,
                Step::Boot
                    | Step::MatchResult
                    | Step::CaptureResult
                    | Step::SaveResult
                    | Step::DeleteResult
                    | Step::Finger
                    | Step::CountBefore
                    | Step::CountAfter
                    | Step::Slot(_)
            ) {
                self.schedule(self.step, QUERY_INTERVAL_MS);
            } else {
                self.fail(Failure::Unavailable, false);
            }
            return;
        }
        if reply.completion != Completion::OK {
            self.module_error(reply.completion);
            return;
        }
        match (self.step, reply.data) {
            (Step::Boot, Data::Count(_)) => {
                self.power.started();
                self.terminal = false;
            }
            (Step::Sleep, Data::None) => {
                if self.stream.power_off().is_err() {
                    self.fail(Failure::Communication, false);
                    return;
                }
                self.temporary = None;
                self.power.stopped();
                self.runtime.end();
            }
            (Step::Match, Data::None) => self.schedule(Step::MatchResult, QUERY_INTERVAL_MS),
            (Step::MatchResult, Data::Match { matched, slot, .. }) => {
                if slot >= MODULE_SLOTS as u16 && matched {
                    self.wire_fault(WireFailure::Protocol(Error::Parameter));
                    return;
                }
                if let Lifecycle::Active(attempt) = self.operations.state() {
                    self.operations.mark_matching(attempt, Matching::Started);
                }
                self.terminal = true;
                self.outcome = Some(Outcome::Verify(if matched {
                    Ok(Match {
                        slot,
                        interaction: true,
                    })
                } else {
                    Err(Failure::NoMatch)
                }));
            }
            (Step::Finger, Data::Finger(true)) => self.schedule(Step::Finger, QUERY_INTERVAL_MS),
            (Step::Finger, Data::Finger(false)) => {
                let step = if matches!(self.purpose, Purpose::Verify) {
                    Step::Match
                } else {
                    Step::Capture(self.temporary.map_or(0, |t| t.next))
                };
                self.schedule(step, 0);
            }
            (Step::Capture(_), Data::None) => self.schedule(Step::CaptureResult, QUERY_INTERVAL_MS),
            (Step::CaptureResult, Data::Capture { .. }) => self.captured(),
            (Step::Save, Data::None) => self.schedule(Step::SaveResult, QUERY_INTERVAL_MS),
            (Step::SaveResult, Data::Saved(slot))
                if self.temporary.is_some_and(|t| t.slot == slot) =>
            {
                self.temporary = None;
                self.sample(Feedback::Good, 0, true);
            }
            (Step::Delete, Data::None) => self.schedule(Step::DeleteResult, QUERY_INTERVAL_MS),
            (Step::DeleteResult, Data::None) => self.schedule(Step::CountBefore, 0),
            (Step::CountBefore, Data::Count(count)) => {
                self.inventory = Inventory::default();
                self.inventory_count = count;
                self.schedule(Step::Slot(0), 0);
            }
            (Step::Slot(slot), Data::Slot { present, .. }) => {
                self.inventory.set(slot, present);
                self.schedule(
                    if slot + 1 < MODULE_SLOTS as u16 {
                        Step::Slot(slot + 1)
                    } else {
                        Step::CountAfter
                    },
                    0,
                );
            }
            (Step::CountAfter, Data::Count(count)) => self.inventory_complete(count),
            _ => self.wire_fault(WireFailure::Protocol(Error::Parameter)),
        }
    }

    fn captured(&mut self) {
        let Some(mut temporary) = self.temporary else {
            self.fail(Failure::Communication, false);
            return;
        };
        temporary.next += 1;
        self.temporary = Some(temporary);
        if temporary.next > ENROLL_SAMPLES {
            self.schedule(Step::Save, 0);
        } else {
            self.sample(Feedback::Good, ENROLL_SAMPLES + 1 - temporary.next, false);
        }
    }

    fn sample(&mut self, feedback: Feedback, remaining: u8, saved: bool) {
        self.terminal = true;
        self.outcome = Some(Outcome::Management(Ok(bio_sensor::Result::Sample {
            feedback,
            remaining,
            saved,
        })));
    }

    fn inventory_complete(&mut self, count: u16) {
        let observed = self
            .inventory
            .0
            .iter()
            .map(|b| b.count_ones() as u16)
            .sum::<u16>();
        if count != self.inventory_count || count != observed {
            self.fail(Failure::Communication, true);
            return;
        }
        let result = match self.purpose {
            Purpose::Management(Command::Inventory) => {
                bio_sensor::Result::Inventory(self.inventory)
            }
            Purpose::Management(Command::Delete { slot }) if !self.inventory.contains(slot) => {
                if self.temporary.is_some_and(|t| t.slot == slot) {
                    self.temporary = None;
                }
                bio_sensor::Result::Deleted
            }
            Purpose::Management(Command::Clear) if self.inventory.empty() => {
                self.temporary = None;
                bio_sensor::Result::Cleared
            }
            _ => {
                self.fail(Failure::Communication, true);
                return;
            }
        };
        self.terminal = true;
        self.outcome = Some(Outcome::Management(Ok(result)));
    }

    fn module_error(&mut self, completion: Completion) {
        self.diagnostic = Some(Diagnostic {
            request: self.wire_request(),
            progress: self.last_progress,
            failure: WireFailure::Module(completion),
        });
        if matches!(self.step, Step::Boot | Step::Sleep) {
            self.fail(Failure::Communication, false);
            return;
        }
        if matches!(self.step, Step::CaptureResult) {
            let feedback = match completion.0 {
                8 => Some(Feedback::NoActivity),
                9 | 0x0e | 0x11 | 0x12 | 0x14 => Some(Feedback::PoorQuality),
                0x0f => Some(Feedback::Exists),
                0x10 => Some(Feedback::TooShort),
                _ => None,
            };
            if let Some(feedback) = feedback {
                let remaining = self
                    .temporary
                    .map_or(ENROLL_SAMPLES, |t| ENROLL_SAMPLES + 1 - t.next);
                self.sample(feedback, remaining, false);
                return;
            }
        }
        let failure =
            if matches!(self.step, Step::MatchResult) && completion == Completion::NO_FINGER {
                if let Lifecycle::Active(attempt) = self.operations.state() {
                    self.operations
                        .mark_matching(attempt, Matching::NotAttempted);
                }
                Failure::InputTimeout
            } else if matches!(self.step, Step::MatchResult)
                && matches!(completion.0, 9 | 10 | 0x0e | 0x10)
            {
                Failure::NoMatch
            } else {
                Failure::Communication
            };
        self.fail(failure, true);
    }

    fn drain_reply(&mut self, reply: Reply) {
        match self.step {
            Step::Cancel if reply.completion == Completion::OK => {
                let result = match self.purpose {
                    Purpose::Verify => Request::MatchResult,
                    _ => Request::CaptureResult,
                };
                self.schedule(Step::CancelledResult(result), 0);
            }
            Step::Cancel => self.fail(Failure::Unavailable, false),
            Step::CancelledResult(_) if reply.completion == Completion::BUSY => {
                self.schedule(self.step, QUERY_INTERVAL_MS);
            }
            Step::CancelledResult(_)
                if matches!(reply.completion, Completion::OK | Completion::INTERRUPTED) =>
            {
                self.complete_drain();
            }
            Step::CancelledResult(_) => self.fail(Failure::Unavailable, false),
            Step::Match | Step::Capture(_) if reply.completion == Completion::OK => {
                self.schedule(Step::Cancel, 0)
            }
            Step::MatchResult | Step::CaptureResult if reply.completion == Completion::BUSY => {
                self.schedule(Step::Cancel, 0)
            }
            Step::Save if reply.completion == Completion::OK => self.schedule(Step::SaveResult, 0),
            Step::Delete if reply.completion == Completion::OK => {
                self.schedule(Step::DeleteResult, 0)
            }
            Step::SaveResult | Step::DeleteResult if reply.completion == Completion::BUSY => {
                self.schedule(self.step, QUERY_INTERVAL_MS);
            }
            _ => self.complete_drain(),
        }
    }

    fn begin(&mut self, channel: u32, purpose: Purpose, step: Step) -> Result<Attempt, Failure> {
        if !self.ready() {
            return Err(Failure::Unavailable);
        }
        let attempt = self.operations.begin(channel, self.runtime.now_ms())?;
        self.purpose = purpose;
        self.outcome = None;
        self.diagnostic = None;
        self.terminal = false;
        self.runtime.processing();
        self.schedule(step, 0);
        Ok(attempt)
    }

    fn idle_input(&mut self) {
        let mut bytes = [0; STEP_BYTES];
        match self.stream.read(&mut bytes) {
            Poll::Pending => {}
            Poll::Ready(Ok(n))
                if self.power.phase() == Phase::Starting
                    && self.exchange.is_none()
                    && n > 0
                    && n <= bytes.len()
                    && bytes[..n].iter().all(|b| *b == 0x55) => {}
            _ => self.fail(Failure::Communication, false),
        }
    }
}

impl<U: Stream, C: Runtime> Sensor for Driver<U, C> {
    fn now_ms(&self) -> u64 {
        self.runtime.now_ms()
    }
    fn ready(&self) -> bool {
        self.healthy
            && self.power.phase() == Phase::Running
            && self.operations.state() == Lifecycle::Idle
    }
    fn start(&mut self, channel: u32) -> Result<Attempt, Failure> {
        self.begin(channel, Purpose::Verify, Step::Finger)
    }
    fn poll(&mut self) -> Poll<Observation> {
        self.pump();
        let Lifecycle::Active(attempt) = self.operations.state() else {
            return Poll::Pending;
        };
        match self.outcome.take() {
            Some(Outcome::Verify(result)) => Poll::Ready(Observation { attempt, result }),
            other => {
                self.outcome = other;
                Poll::Pending
            }
        }
    }
    fn cancelled(&self) -> bool {
        self.runtime.cancelled()
    }
    fn matching(&self) -> Matching {
        self.operations.matching()
    }
    fn wait(&mut self) {
        self.runtime.wait();
    }
    fn finish(&mut self, attempt: Attempt) {
        if self.operations.state() != Lifecycle::Active(attempt) {
            return;
        }
        self.operations.finish(attempt, self.runtime.now_ms());
        self.outcome = None;
        self.runtime.end();
        if self.terminal && self.healthy {
            self.complete_drain();
            return;
        }
        self.temporary = None;
        if self.exchange.as_ref().is_none_or(|e| e.progress.sent == 0) {
            self.exchange = None;
            match self.step {
                Step::MatchResult | Step::CaptureResult => self.schedule(Step::Cancel, 0),
                Step::SaveResult | Step::DeleteResult => self.schedule(self.step, 0),
                _ => self.complete_drain(),
            }
        }
    }
    fn service_idle(&mut self) {
        if self.power.phase() == Phase::Off || !self.healthy {
            return;
        }
        if self.power.phase() == Phase::Starting {
            if self.exchange.is_none() {
                self.idle_input();
            }
            self.pump();
        } else if matches!(self.operations.state(), Lifecycle::Draining { .. })
            || self.power.phase() == Phase::Stopping
        {
            self.pump();
        } else if self.operations.state() == Lifecycle::Idle && self.healthy {
            self.idle_input();
            if self.healthy && self.power.stop(self.runtime.now_ms(), true) {
                self.purpose = Purpose::Sleep;
                self.runtime.processing();
                self.schedule(Step::Sleep, 0);
            }
        }
    }
    fn prepare(&mut self, deadline_ms: u64) -> Result<(), rsk_fido::CtapError> {
        let deadline_ms = self.deadline_ms(deadline_ms);
        self.request_deadline_ms = Some(deadline_ms);
        let start = self.runtime.now_ms();
        self.runtime.processing();
        let result = (|| {
            loop {
                let now_ms = self.runtime.now_ms();
                if self.runtime.cancelled() {
                    if self.power.phase() == Phase::Starting {
                        self.fail(Failure::Unavailable, false);
                    }
                    return Err(rsk_fido::CtapError::KeepAliveCancel);
                }
                if now_ms < start || now_ms >= deadline_ms {
                    if self.power.phase() == Phase::Starting {
                        self.fail(Failure::Timeout, false);
                    }
                    return Err(rsk_fido::CtapError::UserActionTimeout);
                }
                if !self.healthy {
                    return Err(rsk_fido::CtapError::Other);
                }
                if self.power.wake(now_ms, deadline_ms) {
                    if self.stream.power_on().is_err() {
                        self.fail(Failure::Communication, false);
                        return Err(rsk_fido::CtapError::Other);
                    }
                    self.purpose = Purpose::Boot;
                    self.terminal = false;
                    self.schedule(Step::Boot, BOOT_DELAY_MS);
                }
                self.service_idle();
                if self.ready() {
                    return Ok(());
                }
                self.runtime.wait();
            }
        })();
        self.runtime.end();
        result
    }
    fn release(&mut self, enrollment: bool) {
        self.power.release(enrollment);
        self.request_deadline_ms = None;
    }
    fn deadline_ms(&self, deadline_ms: u64) -> u64 {
        self.request_deadline_ms
            .map_or(deadline_ms, |old| old.min(deadline_ms))
    }
    fn capabilities(&self) -> Option<Capabilities> {
        if self.power.phase() != Phase::Unavailable {
            Some(Capabilities {
                samples: ENROLL_SAMPLES,
                slots: MODULE_SLOTS as u16,
            })
        } else {
            None
        }
    }
    fn start_management(&mut self, channel: u32, command: Command) -> Result<Attempt, Failure> {
        let step = match command {
            Command::Inventory => Step::CountBefore,
            Command::Delete { slot } if slot < MODULE_SLOTS as u16 => Step::Delete,
            Command::Clear => Step::Delete,
            Command::Capture {
                slot,
                generation,
                first,
                remaining,
            } if slot < MODULE_SLOTS as u16
                && generation > 0
                && (1..=ENROLL_SAMPLES).contains(&remaining) =>
            {
                if first {
                    if remaining != ENROLL_SAMPLES {
                        return Err(Failure::Communication);
                    }
                } else if !self.temporary.is_some_and(|t| {
                    t.slot == slot
                        && t.generation == generation
                        && ENROLL_SAMPLES + 1 - t.next == remaining
                }) {
                    return Err(Failure::Communication);
                }
                if !self.ready() {
                    return Err(Failure::Unavailable);
                }
                if first {
                    self.temporary = Some(Temporary {
                        slot,
                        generation,
                        next: 1,
                    });
                }
                if first {
                    Step::Capture(1)
                } else {
                    Step::Finger
                }
            }
            _ => return Err(Failure::Communication),
        };
        self.begin(channel, Purpose::Management(command), step)
    }
    fn poll_management(&mut self) -> Poll<bio_sensor::Observation> {
        self.pump();
        let Lifecycle::Active(attempt) = self.operations.state() else {
            return Poll::Pending;
        };
        match self.outcome.take() {
            Some(Outcome::Management(result)) => {
                Poll::Ready(bio_sensor::Observation { attempt, result })
            }
            other => {
                self.outcome = other;
                Poll::Pending
            }
        }
    }
}

#[cfg(test)]
#[path = "fingerprint_driver_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "fingerprint_power_driver_tests.rs"]
mod power_tests;

#[cfg(test)]
#[path = "fingerprint_presence_tests.rs"]
mod presence_tests;
