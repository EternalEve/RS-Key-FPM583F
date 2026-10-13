// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Fingerprint results are inputs to FIDO policy, never authorization by themselves.

use core::task::Poll;

use rsk_fs::{Fs, Storage};

use crate::consts::{EF_PIN, EF_UV_STATE, MAX_UV_RETRIES};
use crate::{CtapError, Ctx, Rng};

pub const VERIFY_TIMEOUT_MS: u64 = 30_000;
pub const DRAIN_TIMEOUT_MS: u64 = 2_000;
const STATE_VERSION: u8 = 1;
const STATE_LEN: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Unavailable,
    NoMatch,
    InputTimeout,
    Timeout,
    Declined,
    Communication,
}

impl Failure {
    pub(crate) fn ctap(self) -> CtapError {
        match self {
            Self::NoMatch => CtapError::UvInvalid,
            Self::InputTimeout | Self::Timeout => CtapError::UserActionTimeout,
            Self::Declined => CtapError::OperationDenied,
            Self::Unavailable | Self::Communication => CtapError::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attempt {
    pub id: u64,
    pub channel: u32,
    pub started_ms: u64,
    pub deadline_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Match {
    pub slot: u16,
    pub interaction: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Observation {
    pub attempt: Attempt,
    pub result: Result<Match, Failure>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    Idle,
    Active(Attempt),
    Draining { attempt: Attempt, deadline_ms: u64 },
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Matching {
    NotStarted,
    Started,
    Unknown,
    NotAttempted,
}

pub struct Operations {
    next_id: u64,
    state: Lifecycle,
    matching: Matching,
}

impl Default for Operations {
    fn default() -> Self {
        Self::new()
    }
}

impl Operations {
    pub const fn new() -> Self {
        Self {
            next_id: 0,
            state: Lifecycle::Idle,
            matching: Matching::NotStarted,
        }
    }

    pub fn state(&self) -> Lifecycle {
        self.state
    }

    pub fn matching(&self) -> Matching {
        self.matching
    }

    pub fn mark_matching(&mut self, attempt: Attempt, matching: Matching) {
        if self.state == Lifecycle::Active(attempt)
            && matching != Matching::NotStarted
            && (self.matching != Matching::Started || matching == Matching::Started)
        {
            self.matching = matching;
        }
    }

    pub fn begin(&mut self, channel: u32, now_ms: u64) -> Result<Attempt, Failure> {
        if self.state != Lifecycle::Idle {
            return Err(Failure::Unavailable);
        }
        let id = self.next_id.checked_add(1).ok_or(Failure::Unavailable)?;
        let deadline_ms = now_ms
            .checked_add(VERIFY_TIMEOUT_MS)
            .ok_or(Failure::Unavailable)?;
        let attempt = Attempt {
            id,
            channel,
            started_ms: now_ms,
            deadline_ms,
        };
        self.next_id = id;
        self.state = Lifecycle::Active(attempt);
        self.matching = Matching::NotStarted;
        Ok(attempt)
    }

    pub fn accepts(&self, attempt: Attempt, now_ms: u64) -> bool {
        self.state == Lifecycle::Active(attempt)
            && now_ms >= attempt.started_ms
            && now_ms < attempt.deadline_ms
    }

    pub fn finish(&mut self, attempt: Attempt, now_ms: u64) {
        if self.state == Lifecycle::Active(attempt) {
            self.state = Lifecycle::Draining {
                attempt,
                deadline_ms: now_ms.saturating_add(DRAIN_TIMEOUT_MS),
            };
        }
    }

    /// Only a confirmed idle module and an empty old-response path may call this.
    pub fn drained(&mut self, attempt: Attempt) -> bool {
        if matches!(self.state, Lifecycle::Draining { attempt: old, .. } if old == attempt) {
            self.state = Lifecycle::Idle;
            true
        } else {
            false
        }
    }

    pub fn expire_drain(&mut self, now_ms: u64) {
        if matches!(self.state, Lifecycle::Draining { deadline_ms, .. } if now_ms >= deadline_ms) {
            self.state = Lifecycle::Unavailable;
        }
    }
}

/// The worker owns this service; IRQ code only moves bytes and cancellation flags.
pub trait Sensor {
    fn now_ms(&self) -> u64;
    fn ready(&self) -> bool;
    fn start(&mut self, channel: u32) -> Result<Attempt, Failure>;
    fn poll(&mut self) -> Poll<Observation>;
    fn cancelled(&self) -> bool;
    fn matching(&self) -> Matching;
    fn wait(&mut self);
    fn finish(&mut self, attempt: Attempt);
    fn service_idle(&mut self);
    fn prepare(&mut self, _deadline_ms: u64) -> Result<(), CtapError> {
        self.service_idle();
        if self.cancelled() {
            Err(CtapError::KeepAliveCancel)
        } else if self.ready() {
            Ok(())
        } else {
            Err(CtapError::Other)
        }
    }
    fn release(&mut self, _enrollment: bool) {}
    fn deadline_ms(&self, deadline_ms: u64) -> u64 {
        deadline_ms
    }
    fn capabilities(&self) -> Option<crate::bio_sensor::Capabilities> {
        None
    }
    fn start_management(
        &mut self,
        _channel: u32,
        _command: crate::bio_sensor::Command,
    ) -> Result<Attempt, Failure> {
        Err(Failure::Unavailable)
    }
    fn poll_management(&mut self) -> Poll<crate::bio_sensor::Observation> {
        Poll::Pending
    }
}

/// Reads use the already-borrowed FIDO store, never a second RefCell borrow.
pub trait Bindings<S: Storage> {
    fn management(&self) -> bool {
        false
    }
    fn configuration(&mut self, fs: &mut Fs<S>) -> Result<Option<u32>, CtapError>;
    fn authorizes(&mut self, fs: &mut Fs<S>, slot: u16, generation: u32)
    -> Result<bool, CtapError>;
}

pub struct Context<'a, S: Storage> {
    pub sensor: &'a mut dyn Sensor,
    pub bindings: &'a mut dyn Bindings<S>,
}

pub fn retries<S: Storage>(fs: &mut Fs<S>) -> Result<Option<u8>, CtapError> {
    let mut record = [0; STATE_LEN + 1];
    match fs
        .try_read(EF_UV_STATE, &mut record)
        .map_err(|_| CtapError::Other)?
    {
        None => Ok(None),
        Some(STATE_LEN)
            if record[0] == STATE_VERSION
                && record[1] <= MAX_UV_RETRIES
                && record[2] == !record[1] =>
        {
            Ok(Some(record[1]))
        }
        Some(_) => Err(CtapError::Other),
    }
}

fn store_retries<S: Storage>(fs: &mut Fs<S>, remaining: u8) -> Result<(), CtapError> {
    fs.put(EF_UV_STATE, &[STATE_VERSION, remaining, !remaining])
        .map_err(|_| CtapError::Other)?;
    if retries(fs)? != Some(remaining) {
        return Err(CtapError::Other);
    }
    Ok(())
}

/// Provision only before any binding is authorized; missing state is otherwise closed.
pub fn initialize_unconfigured<S: Storage>(
    fs: &mut Fs<S>,
    bindings: &mut dyn Bindings<S>,
) -> Result<(), CtapError> {
    if bindings.configuration(fs)?.is_some() {
        return Err(CtapError::NotAllowed);
    }
    if retries(fs)?.is_none() {
        store_retries(fs, MAX_UV_RETRIES)?;
    }
    Ok(())
}

pub(crate) fn restore_after_pin<S: Storage>(fs: &mut Fs<S>) -> Result<(), CtapError> {
    if retries(fs)?.is_some() {
        store_retries(fs, MAX_UV_RETRIES)?;
    }
    Ok(())
}

pub(crate) fn configured<S: Storage, R: Rng>(ctx: &mut Ctx<S, R>) -> Result<bool, CtapError> {
    let Some(fp) = ctx.fingerprint.as_mut() else {
        return Ok(false);
    };
    Ok(fp.bindings.configuration(ctx.fs)?.is_some())
}

fn check_attempt<S: Storage>(
    fs: &mut Fs<S>,
    fp: &mut Context<'_, S>,
) -> Result<(u32, u8), CtapError> {
    let generation = fp
        .bindings
        .configuration(fs)?
        .ok_or(CtapError::NotAllowed)?;
    let mut pin = [0; crate::clientpin::PIN_FILE_LEN + 1];
    match fs
        .try_read(EF_PIN, &mut pin)
        .map_err(|_| CtapError::Other)?
    {
        Some(crate::clientpin::PIN_FILE_LEN) if pin[0] == 0 => {
            store_retries(fs, 0)?;
            return Err(CtapError::UvBlocked);
        }
        Some(crate::clientpin::PIN_FILE_LEN) => {}
        None => return Err(CtapError::NotAllowed),
        Some(_) => return Err(CtapError::Other),
    }
    let remaining = retries(fs)?.ok_or(CtapError::NotAllowed)?;
    if remaining == 0 {
        return Err(CtapError::UvBlocked);
    }
    Ok((generation, remaining))
}

struct Verification {
    attempt: Attempt,
    deadline_ms: u64,
    generation: u32,
    remaining: u8,
    may_have_matched: bool,
}

impl Verification {
    fn sample(&mut self, sensor: &dyn Sensor, now_ms: &mut u64) {
        *now_ms = sensor.now_ms();
        match sensor.matching() {
            Matching::NotAttempted => self.may_have_matched = false,
            Matching::NotStarted => {}
            Matching::Started | Matching::Unknown => self.may_have_matched = true,
        }
    }

    fn invalid_match(&self) -> CtapError {
        if self.remaining == 1 {
            CtapError::UvBlocked
        } else {
            CtapError::UvInvalid
        }
    }

    fn expire_and_refund<S: Storage>(&self, fs: &mut Fs<S>, now_ms: u64) -> Result<(), CtapError> {
        if now_ms < self.attempt.started_ms {
            return Err(CtapError::Other);
        }
        if now_ms >= self.deadline_ms {
            if !self.may_have_matched {
                store_retries(fs, self.remaining)?;
            }
            return Err(CtapError::UserActionTimeout);
        }
        Ok(())
    }
}

fn wait_for_match<S: Storage>(
    fs: &mut Fs<S>,
    fp: &mut Context<'_, S>,
    channel: u32,
    now_ms: &mut u64,
    verification: &mut Verification,
) -> Result<Match, CtapError> {
    loop {
        verification.sample(fp.sensor, now_ms);
        if fp.sensor.cancelled() {
            return Err(CtapError::KeepAliveCancel);
        }
        if verification.attempt.channel != channel {
            return Err(CtapError::Other);
        }
        verification.expire_and_refund(fs, *now_ms)?;
        let observation = match fp.sensor.poll() {
            Poll::Pending => {
                fp.sensor.wait();
                continue;
            }
            Poll::Ready(observation) => observation,
        };
        if observation.attempt != verification.attempt {
            return Err(CtapError::UvInvalid);
        }
        verification.sample(fp.sensor, now_ms);
        verification.expire_and_refund(fs, *now_ms)?;
        return match observation.result {
            Err(Failure::InputTimeout) if !verification.may_have_matched => {
                store_retries(fs, verification.remaining)?;
                Err(CtapError::UserActionTimeout)
            }
            Err(Failure::NoMatch) => Err(verification.invalid_match()),
            Err(failure) => Err(failure.ctap()),
            Ok(matched) => Ok(matched),
        };
    }
}

fn accept_match_and_restore<S: Storage>(
    fs: &mut Fs<S>,
    fp: &mut Context<'_, S>,
    now_ms: &mut u64,
    verification: &Verification,
    matched: Match,
) -> Result<bool, CtapError> {
    if !fp
        .bindings
        .authorizes(fs, matched.slot, verification.generation)?
    {
        return Err(verification.invalid_match());
    }
    if fp.sensor.cancelled() {
        return Err(CtapError::KeepAliveCancel);
    }
    if fp.sensor.now_ms() >= verification.deadline_ms {
        return Err(CtapError::UserActionTimeout);
    }
    store_retries(fs, MAX_UV_RETRIES)?;
    if fp.sensor.cancelled() {
        return Err(CtapError::KeepAliveCancel);
    }
    *now_ms = fp.sensor.now_ms();
    if *now_ms >= verification.deadline_ms || *now_ms < verification.attempt.started_ms {
        return Err(CtapError::UserActionTimeout);
    }
    Ok(matched.interaction)
}

pub(crate) fn refresh_token<S: Storage, R: Rng>(ctx: &mut Ctx<S, R>) -> Result<(), CtapError> {
    if !ctx.state.paut.fingerprint_verified {
        return Ok(());
    }
    if let Err(error) = crate::bio_enrollment::refresh(ctx) {
        ctx.state.stop_using_token();
        return Err(error);
    }
    ctx.state.expire_stale_token(ctx.now_ms);
    ctx.state.expire_user_presence(ctx.now_ms);
    Ok(())
}

pub(crate) fn verify<S: Storage, R: Rng>(ctx: &mut Ctx<S, R>) -> Result<bool, CtapError> {
    let deadline_ms = ctx.now_ms.saturating_add(VERIFY_TIMEOUT_MS);
    check_attempt(
        ctx.fs,
        ctx.fingerprint
            .as_mut()
            .ok_or(CtapError::UnsupportedOption)?,
    )?;
    crate::bio_enrollment::refresh_until(ctx, deadline_ms)?;
    let fp = ctx
        .fingerprint
        .as_mut()
        .ok_or(CtapError::UnsupportedOption)?;
    let (generation, remaining) = check_attempt(ctx.fs, fp)?;
    fp.sensor.prepare(deadline_ms)?;
    ctx.now_ms = fp.sensor.now_ms();
    store_retries(ctx.fs, remaining - 1)?;
    let attempt = fp.sensor.start(ctx.state.channel).map_err(Failure::ctap)?;
    let mut verification = Verification {
        attempt,
        deadline_ms: fp.sensor.deadline_ms(deadline_ms).min(attempt.deadline_ms),
        generation,
        remaining,
        may_have_matched: false,
    };
    let result = wait_for_match(
        ctx.fs,
        fp,
        ctx.state.channel,
        &mut ctx.now_ms,
        &mut verification,
    )
    .and_then(|matched| {
        accept_match_and_restore(ctx.fs, fp, &mut ctx.now_ms, &verification, matched)
    });
    fp.sensor.finish(attempt);
    if result.is_err() {
        ctx.state.stop_using_token();
    }
    result
}

#[cfg(test)]
#[path = "fingerprint_tests.rs"]
mod tests;

#[cfg(kani)]
#[path = "fingerprint_kani.rs"]
mod proofs;
