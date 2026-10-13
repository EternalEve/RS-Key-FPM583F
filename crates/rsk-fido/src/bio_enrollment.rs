// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Standard 0x09 requests; authentication covers the original CBOR parameters.

use minicbor::{Decoder, Encoder};
use rsk_crypto::sha256;
use rsk_fs::Storage;

use crate::bio_directory::{self, Stage};
use crate::bio_sensor::{Capabilities, SESSION_TIMEOUT_MS};
use crate::cbordec::{cbor, def_map, skip_value};
use crate::consts::{BIO_ID_LEN, BIO_NAME_MAX, EF_PIN, MAX_RAW_SUBPARA};
use crate::state::PERM_BE;
use crate::{CtapError, CtapResult, Ctx, Rng};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Subcommand {
    Begin,
    Capture,
    Cancel,
    Enumerate,
    Rename,
    Delete,
    SensorInfo,
}

impl Subcommand {
    fn parse(value: u32) -> Result<Self, CtapError> {
        match value {
            1 => Ok(Self::Begin),
            2 => Ok(Self::Capture),
            3 => Ok(Self::Cancel),
            4 => Ok(Self::Enumerate),
            5 => Ok(Self::Rename),
            6 => Ok(Self::Delete),
            7 => Ok(Self::SensorInfo),
            _ => Err(CtapError::InvalidSubcommand),
        }
    }
}

pub(crate) struct Request<'a> {
    pub subcommand: Option<Subcommand>,
    pub raw: &'a [u8],
    pub id: Option<&'a [u8]>,
    pub name: Option<&'a str>,
    pub timeout: u64,
    command_byte: u8,
    modality: Option<u32>,
    get_modality: bool,
    protocol: Option<u64>,
    auth: Option<&'a [u8]>,
}

#[derive(Clone, Copy)]
pub struct Session {
    pub slot: u16,
    pub channel: u32,
    pub generation: u32,
    pub id: [u8; BIO_ID_LEN],
    pub remaining: u8,
    pub token_hash: [u8; 32],
    pub deadline_ms: u64,
}

fn parse(data: &[u8]) -> Result<Request<'_>, CtapError> {
    let mut d = Decoder::new(data);
    let mut req = Request {
        subcommand: None,
        raw: &[],
        id: None,
        name: None,
        timeout: SESSION_TIMEOUT_MS,
        command_byte: 0,
        modality: None,
        get_modality: false,
        protocol: None,
        auth: None,
    };
    for _ in 0..def_map(&mut d)? {
        match cbor(d.u32())? {
            1 => req.modality = Some(cbor(d.u32())?),
            2 => {
                let byte = cbor(d.u32())?;
                req.subcommand = Some(Subcommand::parse(byte)?);
                req.command_byte = byte as u8;
            }
            3 => {
                let start = d.position();
                for _ in 0..def_map(&mut d)? {
                    match cbor(d.u32())? {
                        1 => req.id = Some(cbor(d.bytes())?),
                        2 => req.name = Some(cbor(d.str())?),
                        3 => req.timeout = cbor(d.u32())? as u64,
                        _ => skip_value(&mut d)?,
                    }
                }
                req.raw = &data[start..d.position()];
            }
            4 => req.protocol = Some(cbor(d.u32())? as u64),
            5 => req.auth = Some(cbor(d.bytes())?),
            6 => {
                req.get_modality = cbor(d.bool())?;
                if !req.get_modality {
                    return Err(CtapError::InvalidParameter);
                }
            }
            _ => skip_value(&mut d)?,
        }
    }
    Ok(req)
}

pub fn supported<S: Storage, R: Rng>(ctx: &Ctx<S, R>) -> bool {
    ctx.fingerprint
        .as_ref()
        .is_some_and(|fp| fp.bindings.management())
}

pub fn refresh<S: Storage, R: Rng>(ctx: &mut Ctx<S, R>) -> Result<(), CtapError> {
    refresh_until(ctx, ctx.now_ms.saturating_add(SESSION_TIMEOUT_MS))
}

pub(crate) fn expire_session(state: &mut crate::FidoState, now_ms: u64) {
    state.expire_stale_token(now_ms);
    if state.enrollment.is_some_and(|s| {
        now_ms >= s.deadline_ms || sha256(&state.paut.token) != s.token_hash || !state.paut.in_use
    }) {
        state.enrollment = None;
    }
}

pub(crate) fn refresh_until<S: Storage, R: Rng>(
    ctx: &mut Ctx<S, R>,
    deadline_ms: u64,
) -> Result<(), CtapError> {
    if !supported(ctx) {
        return Ok(());
    }
    expire_session(ctx.state, ctx.now_ms);
    let fp = ctx.fingerprint.as_mut().ok_or(CtapError::InvalidCommand)?;
    if bio_directory::reset_pending(ctx.fs)? {
        return Ok(());
    }
    let result = crate::bio_sensor::execute_until(
        fp.sensor,
        ctx.state.channel,
        crate::bio_sensor::Command::Inventory,
        deadline_ms,
    );
    ctx.now_ms = fp.sensor.now_ms();
    expire_session(ctx.state, ctx.now_ms);
    let crate::bio_sensor::Result::Inventory(inventory) = result? else {
        return Err(CtapError::Other);
    };
    if bio_directory::reconcile(ctx.fs, inventory, ctx.state.enrollment.map(|s| s.slot))? {
        ctx.state.invalidate_fingerprint_authorization();
    }
    ctx.now_ms = fp.sensor.now_ms();
    Ok(())
}

fn capabilities<S: Storage, R: Rng>(ctx: &Ctx<S, R>) -> Result<Capabilities, CtapError> {
    let caps = ctx
        .fingerprint
        .as_ref()
        .and_then(|fp| fp.sensor.capabilities())
        .ok_or(CtapError::Other)?;
    if caps.samples == 0 || caps.slots == 0 || caps.slots as usize > crate::bio_sensor::MODULE_SLOTS
    {
        return Err(CtapError::Other);
    }
    Ok(caps)
}

fn authorize<S: Storage, R: Rng>(
    ctx: &mut Ctx<S, R>,
    req: &Request<'_>,
    command: Subcommand,
) -> Result<(), CtapError> {
    let auth = req.auth.ok_or(CtapError::PuatRequired)?;
    if req.modality.is_none() || req.protocol.is_none() {
        return Err(CtapError::MissingParameter);
    }
    if matches!(
        command,
        Subcommand::Capture | Subcommand::Rename | Subcommand::Delete
    ) && req.id.is_none()
    {
        return Err(CtapError::MissingParameter);
    }
    if command == Subcommand::Rename && req.name.is_none() {
        return Err(CtapError::MissingParameter);
    }
    let proto =
        crate::clientpin::checked_proto(req.protocol)?.ok_or(CtapError::MissingParameter)?;
    if req.name.is_some_and(|name| name.len() > BIO_NAME_MAX) {
        return Err(CtapError::InvalidLength);
    }
    if !ctx.fs.try_has_data(EF_PIN).map_err(|_| CtapError::Other)? {
        return Err(CtapError::PinNotSet);
    }
    let raw = if matches!(command, Subcommand::Enumerate) {
        &[][..]
    } else {
        req.raw
    };
    if raw.len() > MAX_RAW_SUBPARA {
        return Err(CtapError::LimitExceeded);
    }
    let mut message = [0; 2 + MAX_RAW_SUBPARA];
    message[0] = 1;
    message[1] = req.command_byte;
    message[2..2 + raw.len()].copy_from_slice(raw);
    if !ctx
        .state
        .verify_token(proto, &message[..2 + raw.len()], auth)
        || ctx.state.paut.permissions & PERM_BE == 0
    {
        return Err(CtapError::PinAuthInvalid);
    }
    ctx.state.mark_token_used(ctx.now_ms);
    Ok(())
}

pub fn bio_enrollment<S: Storage, R: Rng>(
    ctx: &mut Ctx<S, R>,
    data: &[u8],
    out: &mut [u8],
) -> CtapResult {
    if !supported(ctx) {
        return Err(CtapError::InvalidCommand);
    }
    let req = parse(data)?;
    if req.get_modality {
        let mut enc = Encoder::new(minicbor::encode::write::Cursor::new(out));
        enc.map(1)
            .and_then(|e| e.u8(1))
            .and_then(|e| e.u8(1))
            .map_err(|_| CtapError::Other)?;
        return Ok(enc.writer().position());
    }
    let command = req.subcommand.ok_or(CtapError::MissingParameter)?;
    if req.modality.is_some_and(|m| m != 1) {
        return Err(CtapError::InvalidParameter);
    }
    if command == Subcommand::SensorInfo {
        if req.modality.is_none() {
            return Err(CtapError::MissingParameter);
        }
        let caps = capabilities(ctx)?;
        let mut enc = Encoder::new(minicbor::encode::write::Cursor::new(out));
        enc.map(3)
            .and_then(|e| e.u8(2))
            .and_then(|e| e.u8(1))
            .and_then(|e| e.u8(3))
            .and_then(|e| e.u8(caps.samples))
            .and_then(|e| e.u8(8))
            .and_then(|e| e.u8(BIO_NAME_MAX as u8))
            .map_err(|_| CtapError::Other)?;
        return Ok(enc.writer().position());
    }
    if command == Subcommand::Cancel {
        if req.modality.is_none() {
            return Err(CtapError::MissingParameter);
        }
        return crate::bio_management::cancel(ctx);
    }
    authorize(ctx, &req, command)?;
    refresh(ctx)?;
    ctx.state.expire_stale_token(ctx.now_ms);
    authorize(ctx, &req, command)?;
    if bio_directory::reset_pending(ctx.fs)? {
        return Err(CtapError::NotAllowed);
    }
    match command {
        Subcommand::Begin => {
            crate::bio_management::begin(ctx, capabilities(ctx)?, req.timeout, out)
        }
        Subcommand::Capture => crate::bio_management::capture(ctx, &req, out),
        Subcommand::Enumerate => enumerate(ctx, out),
        Subcommand::Rename => rename(ctx, &req),
        Subcommand::Delete => {
            crate::bio_management::delete(ctx, req.id.ok_or(CtapError::MissingParameter)?)
        }
        Subcommand::Cancel | Subcommand::SensorInfo => Err(CtapError::InvalidSubcommand),
    }
}

pub(crate) fn find<S: Storage, R: Rng>(
    ctx: &mut Ctx<S, R>,
    id: &[u8],
) -> Result<(u16, bio_directory::Entry), CtapError> {
    bio_directory::read(ctx.fs)?
        .entries
        .iter()
        .enumerate()
        .find_map(|(slot, entry)| {
            entry
                .filter(|e| e.id == id && e.stage != Stage::Retired)
                .map(|e| (slot as u16, e))
        })
        .ok_or(CtapError::InvalidOption)
}

fn enumerate<S: Storage, R: Rng>(ctx: &mut Ctx<S, R>, out: &mut [u8]) -> CtapResult {
    let snapshot = bio_directory::read(ctx.fs)?;
    let entries = snapshot
        .entries
        .iter()
        .flatten()
        .filter(|e| matches!(e.stage, Stage::Authorized | Stage::Saved | Stage::Deleting));
    let count = entries.clone().count();
    if count == 0 {
        return Err(CtapError::InvalidOption);
    }
    let mut enc = Encoder::new(minicbor::encode::write::Cursor::new(out));
    enc.map(1)
        .and_then(|e| e.u8(7))
        .and_then(|e| e.array(count as u64))
        .map_err(|_| CtapError::Other)?;
    for entry in entries {
        let name = entry.name()?;
        enc.map(2)
            .and_then(|e| e.u8(1))
            .and_then(|e| e.bytes(&entry.id))
            .and_then(|e| e.u8(2))
            .and_then(|e| e.str(name))
            .map_err(|_| CtapError::Other)?;
    }
    Ok(enc.writer().position())
}

fn rename<S: Storage, R: Rng>(ctx: &mut Ctx<S, R>, req: &Request<'_>) -> CtapResult {
    let (slot, mut entry) = find(ctx, req.id.ok_or(CtapError::MissingParameter)?)?;
    if entry.stage == Stage::Pending {
        return Err(CtapError::InvalidOption);
    }
    let name = req.name.ok_or(CtapError::MissingParameter)?;
    entry.name.fill(0);
    entry.name[..name.len()].copy_from_slice(name.as_bytes());
    entry.name_len = name.len() as u8;
    bio_directory::write_entry(ctx.fs, slot, entry)?;
    Ok(0)
}

#[cfg(test)]
#[path = "bio_enrollment_tests.rs"]
mod tests;
