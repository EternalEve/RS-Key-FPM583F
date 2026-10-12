// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Durable enrollment and deletion ordering, independent of transport and codec.

use minicbor::Encoder;
use rsk_crypto::sha256;
use rsk_fs::Storage;

use crate::bio_directory::{self, Entry, Head, Stage};
use crate::bio_enrollment::{Request, Session};
use crate::bio_sensor::{self, Capabilities, Command, Feedback, SESSION_TIMEOUT_MS};
use crate::consts::{BIO_ID_LEN, BIO_NAME_MAX, EF_BIO_RESET, MAX_BIO_TEMPLATES};
use crate::{CtapError, CtapResult, Ctx, Rng};

fn run<S: Storage, R: Rng>(
    ctx: &mut Ctx<S, R>,
    command: Command,
    timeout: u64,
) -> Result<bio_sensor::Result, CtapError> {
    let sensor = &mut *ctx
        .fingerprint
        .as_mut()
        .ok_or(CtapError::InvalidCommand)?
        .sensor;
    let result = bio_sensor::execute(
        sensor,
        ctx.state.channel,
        command,
        timeout.min(SESSION_TIMEOUT_MS),
    );
    ctx.now_ms = sensor.now_ms();
    result
}

pub(crate) fn begin<S: Storage, R: Rng>(
    ctx: &mut Ctx<S, R>,
    caps: Capabilities,
    timeout: u64,
    out: &mut [u8],
) -> CtapResult {
    let snapshot = bio_directory::read(ctx.fs)?;
    if snapshot.head.is_some_and(|h| h.isolated) {
        return Err(CtapError::NotAllowed);
    }
    let capacity = (caps.slots as usize).min(MAX_BIO_TEMPLATES);
    if !snapshot.entries[..capacity]
        .iter()
        .any(|e| e.is_none_or(|e| matches!(e.stage, Stage::Retired | Stage::Pending)))
    {
        return Err(CtapError::FpDatabaseFull);
    }
    cancel(ctx)?;
    for (slot, entry) in bio_directory::read(ctx.fs)?.entries.iter().enumerate() {
        if let Some(entry) = entry.filter(|e| e.stage == Stage::Pending) {
            delete_entry(ctx, slot as u16, entry)?;
        }
    }
    let slot = bio_directory::read(ctx.fs)?.entries[..capacity]
        .iter()
        .position(|e| e.is_none_or(|e| e.stage == Stage::Retired))
        .ok_or(CtapError::FpDatabaseFull)? as u16;
    let generation = snapshot
        .head
        .map_or(0, |h| h.generation)
        .checked_add(1)
        .ok_or(CtapError::LimitExceeded)?;
    let retries = crate::fingerprint::retries(ctx.fs)?;
    if snapshot.head.is_some() && retries.is_none() {
        return Err(CtapError::Other);
    }
    bio_directory::write_head(
        ctx.fs,
        Head {
            generation,
            isolated: false,
        },
    )?;
    if snapshot.head.is_none() {
        let fp = ctx.fingerprint.as_mut().ok_or(CtapError::InvalidCommand)?;
        crate::fingerprint::initialize_unconfigured(ctx.fs, fp.bindings)?;
    }

    let mut id = [0; BIO_ID_LEN];
    ctx.rng.fill(&mut id);
    id[BIO_ID_LEN - 4..].copy_from_slice(&generation.to_le_bytes());
    bio_directory::write_entry(
        ctx.fs,
        slot,
        Entry {
            stage: Stage::Pending,
            generation,
            id,
            name: [0; BIO_NAME_MAX],
            name_len: 0,
        },
    )?;
    ctx.state.enrollment = Some(Session {
        slot,
        channel: ctx.state.channel,
        generation,
        id,
        remaining: caps.samples,
        token_hash: sha256(&ctx.state.paut.token),
        deadline_ms: ctx.now_ms.saturating_add(SESSION_TIMEOUT_MS),
    });
    sample(ctx, timeout, true, out)
}

pub(crate) fn capture<S: Storage, R: Rng>(
    ctx: &mut Ctx<S, R>,
    req: &Request<'_>,
    out: &mut [u8],
) -> CtapResult {
    let session = ctx.state.enrollment.ok_or(CtapError::NotAllowed)?;
    if req.id != Some(&session.id)
        || session.token_hash != sha256(&ctx.state.paut.token)
        || ctx.now_ms >= session.deadline_ms
    {
        return Err(CtapError::NotAllowed);
    }
    ctx.state
        .enrollment
        .as_mut()
        .ok_or(CtapError::NotAllowed)?
        .channel = ctx.state.channel;
    sample(ctx, req.timeout, false, out)
}

fn sample<S: Storage, R: Rng>(
    ctx: &mut Ctx<S, R>,
    timeout: u64,
    first: bool,
    out: &mut [u8],
) -> CtapResult {
    let mut session = ctx.state.enrollment.ok_or(CtapError::NotAllowed)?;
    let result = run(
        ctx,
        Command::Capture {
            slot: session.slot,
            generation: session.generation,
            first,
            remaining: session.remaining,
        },
        timeout,
    );
    let sampled = complete_sample(ctx, &mut session, result);
    let (feedback, remaining) = match sampled {
        Ok(sample) => sample,
        Err(error) => {
            ctx.state.enrollment = None;
            return Err(error);
        }
    };
    ctx.state.enrollment = if remaining == 0 {
        None
    } else {
        session.remaining = remaining;
        session.deadline_ms = ctx.now_ms.saturating_add(SESSION_TIMEOUT_MS);
        Some(session)
    };
    ctx.state.mark_token_used(ctx.now_ms);
    let mut enc = Encoder::new(minicbor::encode::write::Cursor::new(out));
    enc.map(if first { 3 } else { 2 })
        .map_err(|_| CtapError::Other)?;
    if first {
        enc.u8(4)
            .and_then(|e| e.bytes(&session.id))
            .map_err(|_| CtapError::Other)?;
    }
    enc.u8(5)
        .and_then(|e| e.u8(feedback as u8))
        .and_then(|e| e.u8(6))
        .and_then(|e| e.u8(remaining))
        .map_err(|_| CtapError::Other)?;
    Ok(enc.writer().position())
}

fn complete_sample<S: Storage, R: Rng>(
    ctx: &mut Ctx<S, R>,
    session: &mut Session,
    result: Result<bio_sensor::Result, CtapError>,
) -> Result<(Feedback, u8), CtapError> {
    let bio_sensor::Result::Sample {
        feedback,
        remaining,
        saved,
    } = result?
    else {
        return Err(CtapError::Other);
    };
    let valid_progress = if feedback == Feedback::Good {
        remaining < session.remaining
    } else {
        remaining == session.remaining
    };
    if !valid_progress
        || saved != (remaining == 0)
        || (remaining == 0 && feedback != Feedback::Good)
    {
        return Err(CtapError::Other);
    }
    if remaining == 0 {
        let (slot, mut entry) = crate::bio_enrollment::find(ctx, &session.id)?;
        if slot != session.slot
            || entry.generation != session.generation
            || entry.stage != Stage::Pending
        {
            return Err(CtapError::Other);
        }
        if ctx
            .fingerprint
            .as_ref()
            .is_some_and(|fp| fp.sensor.cancelled())
        {
            return Err(CtapError::KeepAliveCancel);
        }
        entry.stage = Stage::Saved;
        bio_directory::write_entry(ctx.fs, slot, entry)?;
        let fp = ctx.fingerprint.as_mut().ok_or(CtapError::Other)?;
        fp.sensor.service_idle();
        if !fp.sensor.ready()
            || !bio_sensor::read_inventory(fp.sensor, ctx.state.channel)?.contains(slot)
        {
            return Err(CtapError::Other);
        }
        if fp.sensor.cancelled() {
            return Err(CtapError::KeepAliveCancel);
        }
        ctx.now_ms = fp.sensor.now_ms();
        ctx.state.expire_stale_token(ctx.now_ms);
        if !ctx.state.paut.in_use || session.token_hash != sha256(&ctx.state.paut.token) {
            return Err(CtapError::PinAuthInvalid);
        }
        entry.stage = Stage::Authorized;
        bio_directory::write_entry(ctx.fs, slot, entry)?;
    }
    Ok((feedback, remaining))
}

pub(crate) fn cancel<S: Storage, R: Rng>(ctx: &mut Ctx<S, R>) -> CtapResult {
    let Some(session) = ctx.state.enrollment.take() else {
        return Ok(0);
    };
    let (slot, entry) = crate::bio_enrollment::find(ctx, &session.id)?;
    if slot != session.slot || entry.stage != Stage::Pending {
        return Err(CtapError::Other);
    }
    delete_entry(ctx, slot, entry)?;
    Ok(0)
}

pub(crate) fn delete<S: Storage, R: Rng>(ctx: &mut Ctx<S, R>, id: &[u8]) -> CtapResult {
    let (slot, entry) = crate::bio_enrollment::find(ctx, id)?;
    if ctx.state.enrollment.is_some_and(|s| s.id == id) {
        ctx.state.enrollment = None;
    }
    delete_entry(ctx, slot, entry)?;
    Ok(0)
}

fn delete_entry<S: Storage, R: Rng>(
    ctx: &mut Ctx<S, R>,
    slot: u16,
    mut entry: Entry,
) -> Result<(), CtapError> {
    entry.stage = Stage::Deleting;
    bio_directory::write_entry(ctx.fs, slot, entry)?;
    ctx.state.invalidate_fingerprint_authorization();
    match run(ctx, Command::Delete { slot }, SESSION_TIMEOUT_MS)? {
        bio_sensor::Result::Deleted => {}
        _ => return Err(CtapError::Other),
    }
    let fp = ctx.fingerprint.as_mut().ok_or(CtapError::Other)?;
    fp.sensor.service_idle();
    if !fp.sensor.ready()
        || bio_sensor::read_inventory(fp.sensor, ctx.state.channel)?.contains(slot)
    {
        return Err(CtapError::Other);
    }
    entry.stage = Stage::Retired;
    entry.name.fill(0);
    entry.name_len = 0;
    bio_directory::write_entry(ctx.fs, slot, entry)
}

pub fn prepare_reset<S: Storage, R: Rng>(ctx: &mut Ctx<S, R>) -> Result<bool, CtapError> {
    if !crate::bio_enrollment::supported(ctx) {
        if !bio_directory::configuration_clean(ctx.fs)? {
            return Err(CtapError::NotAllowed);
        }
        return Ok(false);
    }
    bio_directory::begin_reset(ctx.fs)?;
    ctx.state.enrollment = None;
    ctx.state.stop_using_token();
    match run(ctx, Command::Clear, SESSION_TIMEOUT_MS)? {
        bio_sensor::Result::Cleared => {}
        _ => return Err(CtapError::Other),
    }
    let fp = ctx.fingerprint.as_mut().ok_or(CtapError::Other)?;
    fp.sensor.service_idle();
    if !fp.sensor.ready() || !bio_sensor::read_inventory(fp.sensor, ctx.state.channel)?.empty() {
        return Err(CtapError::Other);
    }
    Ok(true)
}

pub fn finish_reset<S: Storage, R: Rng>(ctx: &mut Ctx<S, R>) -> Result<(), CtapError> {
    ctx.fs.delete(EF_BIO_RESET).map_err(|_| CtapError::Other)?;
    if bio_directory::reset_pending(ctx.fs)? {
        return Err(CtapError::Other);
    }
    Ok(())
}
