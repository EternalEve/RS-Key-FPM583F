// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Versioned metadata; one entry append is the sole authorization commit.

use rsk_crypto::crc::crc32;
use rsk_fs::{Fs, Storage};

use crate::CtapError;
use crate::bio_sensor::{Inventory, MODULE_SLOTS};
use crate::consts::{
    BIO_ID_LEN, BIO_NAME_MAX, EF_BIO_DIRECTORY, EF_BIO_RESET, EF_BIO_TEMPLATE, MAX_BIO_TEMPLATES,
};
use crate::fingerprint::Bindings;

const VERSION: u8 = 1;
const HEAD_LEN: usize = 10;
const RESET_LEN: usize = 6;
const ENTRY_PREFIX: usize = 1 + 1 + 4 + BIO_ID_LEN + 1;
pub const ENTRY_MAX: usize = ENTRY_PREFIX + BIO_NAME_MAX + 4;
pub const ENUMERATION_MAX: usize =
    5 + MAX_BIO_TEMPLATES * (1 + 1 + 1 + BIO_ID_LEN + 1 + 2 + BIO_NAME_MAX);
const _: () = assert!(ENUMERATION_MAX < crate::consts::MAX_MSG_SIZE as usize);
const _: () = assert!(ENTRY_MAX < rsk_fs::MAX_VALUE_BYTES);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Stage {
    Pending = 1,
    Saved = 2,
    Authorized = 3,
    Deleting = 4,
    Retired = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub stage: Stage,
    pub generation: u32,
    pub id: [u8; BIO_ID_LEN],
    pub name: [u8; BIO_NAME_MAX],
    pub name_len: u8,
}

impl Entry {
    pub fn name(&self) -> Result<&str, CtapError> {
        let name = self
            .name
            .get(..self.name_len as usize)
            .ok_or(CtapError::Other)?;
        core::str::from_utf8(name).map_err(|_| CtapError::Other)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Head {
    pub generation: u32,
    pub isolated: bool,
}

pub struct Snapshot {
    pub head: Option<Head>,
    pub entries: [Option<Entry>; MAX_BIO_TEMPLATES],
}

fn check(record: &[u8]) -> Result<(), CtapError> {
    let n = record.len().checked_sub(4).ok_or(CtapError::Other)?;
    if record.first() != Some(&VERSION) || record[n..] != crc32(&record[..n]).to_le_bytes() {
        return Err(CtapError::Other);
    }
    Ok(())
}

fn commit_record<S: Storage>(fs: &mut Fs<S>, fid: u16, record: &mut [u8]) -> Result<(), CtapError> {
    let n = record.len() - 4;
    let crc = crc32(&record[..n]).to_le_bytes();
    record[n..].copy_from_slice(&crc);
    fs.put(fid, record).map_err(|_| CtapError::Other)?;
    let mut readback = [0; ENTRY_MAX + 1];
    if fs
        .try_read(fid, &mut readback)
        .map_err(|_| CtapError::Other)?
        != Some(record.len())
        || readback[..record.len()] != *record
    {
        return Err(CtapError::Other);
    }
    Ok(())
}

pub fn read<S: Storage>(fs: &mut Fs<S>) -> Result<Snapshot, CtapError> {
    let mut head_bytes = [0; HEAD_LEN + 1];
    let head = match fs
        .try_read(EF_BIO_DIRECTORY, &mut head_bytes)
        .map_err(|_| CtapError::Other)?
    {
        None => None,
        Some(HEAD_LEN) => {
            check(&head_bytes[..HEAD_LEN])?;
            if head_bytes[1] > 1 {
                return Err(CtapError::Other);
            }
            Some(Head {
                generation: u32::from_le_bytes(
                    head_bytes[2..6].try_into().map_err(|_| CtapError::Other)?,
                ),
                isolated: head_bytes[1] == 1,
            })
        }
        Some(_) => return Err(CtapError::Other),
    };
    let mut snapshot = Snapshot {
        head,
        entries: [None; MAX_BIO_TEMPLATES],
    };
    for slot in 0..MAX_BIO_TEMPLATES {
        let mut bytes = [0; ENTRY_MAX + 1];
        let Some(n) = fs
            .try_read(EF_BIO_TEMPLATE + slot as u16, &mut bytes)
            .map_err(|_| CtapError::Other)?
        else {
            continue;
        };
        if !(ENTRY_PREFIX + 4..=ENTRY_MAX).contains(&n) {
            return Err(CtapError::Other);
        }
        check(&bytes[..n])?;
        let name_len = bytes[ENTRY_PREFIX - 1] as usize;
        if name_len > BIO_NAME_MAX || n != ENTRY_PREFIX + name_len + 4 {
            return Err(CtapError::Other);
        }
        let stage = match bytes[1] {
            1 => Stage::Pending,
            2 => Stage::Saved,
            3 => Stage::Authorized,
            4 => Stage::Deleting,
            5 => Stage::Retired,
            _ => return Err(CtapError::Other),
        };
        let generation = u32::from_le_bytes(bytes[2..6].try_into().map_err(|_| CtapError::Other)?);
        if generation == 0 || head.is_none_or(|h| generation > h.generation) {
            return Err(CtapError::Other);
        }
        let mut entry = Entry {
            stage,
            generation,
            id: [0; BIO_ID_LEN],
            name: [0; BIO_NAME_MAX],
            name_len: name_len as u8,
        };
        entry.id.copy_from_slice(&bytes[6..6 + BIO_ID_LEN]);
        entry.name[..name_len].copy_from_slice(&bytes[ENTRY_PREFIX..ENTRY_PREFIX + name_len]);
        core::str::from_utf8(&entry.name[..name_len]).map_err(|_| CtapError::Other)?;
        if snapshot.entries[..slot]
            .iter()
            .flatten()
            .any(|e| e.id == entry.id || e.generation == generation)
        {
            return Err(CtapError::Other);
        }
        snapshot.entries[slot] = Some(entry);
    }
    let mut seen = 0u32;
    let mut bad = false;
    let complete = fs.for_each_key(&mut |fid| {
        if (EF_BIO_TEMPLATE..EF_BIO_TEMPLATE + MAX_BIO_TEMPLATES as u16).contains(&fid) {
            let bit = 1 << (fid - EF_BIO_TEMPLATE);
            seen |= bit;
        } else if (EF_BIO_TEMPLATE + MAX_BIO_TEMPLATES as u16
            ..EF_BIO_TEMPLATE + MODULE_SLOTS as u16)
            .contains(&fid)
        {
            bad = true;
        }
    });
    let expected = snapshot
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.is_some())
        .fold(0u32, |bits, (slot, _)| bits | (1 << slot));
    if !complete || bad || seen != expected {
        return Err(CtapError::Other);
    }
    Ok(snapshot)
}

pub fn write_head<S: Storage>(fs: &mut Fs<S>, head: Head) -> Result<(), CtapError> {
    if !fs
        .try_has_data(EF_BIO_RESET)
        .map_err(|_| CtapError::Other)?
    {
        commit_record(fs, EF_BIO_RESET, &mut [VERSION, 0, 0, 0, 0, 0])?;
    }
    let mut bytes = [0; HEAD_LEN];
    bytes[0] = VERSION;
    bytes[1] = u8::from(head.isolated);
    bytes[2..6].copy_from_slice(&head.generation.to_le_bytes());
    commit_record(fs, EF_BIO_DIRECTORY, &mut bytes)
}

pub fn write_entry<S: Storage>(fs: &mut Fs<S>, slot: u16, entry: Entry) -> Result<(), CtapError> {
    if slot as usize >= MAX_BIO_TEMPLATES || entry.name_len as usize > BIO_NAME_MAX {
        return Err(CtapError::Other);
    }
    core::str::from_utf8(&entry.name[..entry.name_len as usize]).map_err(|_| CtapError::Other)?;
    let mut bytes = [0; ENTRY_MAX];
    bytes[0] = VERSION;
    bytes[1] = entry.stage as u8;
    bytes[2..6].copy_from_slice(&entry.generation.to_le_bytes());
    bytes[6..6 + BIO_ID_LEN].copy_from_slice(&entry.id);
    bytes[ENTRY_PREFIX - 1] = entry.name_len;
    let n = ENTRY_PREFIX + entry.name_len as usize;
    bytes[ENTRY_PREFIX..n].copy_from_slice(&entry.name[..entry.name_len as usize]);
    commit_record(fs, EF_BIO_TEMPLATE + slot, &mut bytes[..n + 4])
}

pub fn reset_pending<S: Storage>(fs: &mut Fs<S>) -> Result<bool, CtapError> {
    let mut record = [0; RESET_LEN + 1];
    match fs
        .try_read(EF_BIO_RESET, &mut record)
        .map_err(|_| CtapError::Other)?
    {
        None => Ok(false),
        Some(RESET_LEN) => {
            check(&record[..RESET_LEN])?;
            match record[1] {
                0 => Ok(false),
                1 => Ok(true),
                _ => Err(CtapError::Other),
            }
        }
        Some(_) => Err(CtapError::Other),
    }
}

pub fn begin_reset<S: Storage>(fs: &mut Fs<S>) -> Result<(), CtapError> {
    commit_record(fs, EF_BIO_RESET, &mut [VERSION, 1, 0, 0, 0, 0])
}

pub fn configuration_clean<S: Storage>(fs: &mut Fs<S>) -> Result<bool, CtapError> {
    if reset_pending(fs)? {
        return Ok(false);
    }
    // Empty records are malformed state, not evidence of an unconfigured module.
    if fs
        .try_read(EF_BIO_DIRECTORY, &mut [])
        .map_err(|_| CtapError::Other)?
        .is_none()
    {
        let mut orphan = false;
        for slot in 0..MODULE_SLOTS {
            orphan |= fs
                .try_read(EF_BIO_TEMPLATE + slot as u16, &mut [])
                .map_err(|_| CtapError::Other)?
                .is_some();
        }
        if !orphan {
            return Ok(true);
        }
    }
    let snapshot = read(fs)?;
    Ok(snapshot.head.is_none_or(|h| !h.isolated)
        && snapshot
            .entries
            .iter()
            .flatten()
            .all(|e| e.stage == Stage::Retired))
}

pub fn reconcile<S: Storage>(
    fs: &mut Fs<S>,
    inventory: Inventory,
    live_slot: Option<u16>,
) -> Result<bool, CtapError> {
    let snapshot = read(fs)?;
    let mut changed = false;
    if reset_pending(fs)? {
        return Err(CtapError::NotAllowed);
    }
    let mut unknown = false;
    for slot in 0..MODULE_SLOTS {
        if inventory.contains(slot as u16)
            && (slot >= MAX_BIO_TEMPLATES
                || snapshot.entries[slot].is_none_or(|e| e.stage == Stage::Retired))
        {
            unknown = true;
        }
    }
    if unknown {
        changed = true;
        let mut head = snapshot.head.unwrap_or(Head {
            generation: 0,
            isolated: true,
        });
        if !head.isolated || snapshot.head.is_none() {
            head.isolated = true;
            write_head(fs, head)?;
        }
    }
    for (slot, entry) in snapshot.entries.iter().enumerate() {
        let Some(mut entry) = *entry else {
            continue;
        };
        if entry.stage == Stage::Authorized && !inventory.contains(slot as u16) {
            changed = true;
            entry.stage = Stage::Deleting;
            write_entry(fs, slot as u16, entry)?;
        } else if entry.stage == Stage::Pending
            && inventory.contains(slot as u16)
            && live_slot != Some(slot as u16)
        {
            entry.stage = Stage::Saved;
            write_entry(fs, slot as u16, entry)?;
        }
    }
    Ok(changed)
}

pub struct Directory;

impl<S: Storage> Bindings<S> for Directory {
    fn management(&self) -> bool {
        true
    }
    fn configuration(&mut self, fs: &mut Fs<S>) -> Result<Option<u32>, CtapError> {
        if reset_pending(fs)? {
            return Ok(None);
        }
        let snapshot = read(fs)?;
        Ok(snapshot
            .head
            .filter(|h| !h.isolated)
            .filter(|_| {
                snapshot
                    .entries
                    .iter()
                    .flatten()
                    .any(|e| e.stage == Stage::Authorized)
            })
            .map(|h| h.generation))
    }
    fn authorizes(
        &mut self,
        fs: &mut Fs<S>,
        slot: u16,
        generation: u32,
    ) -> Result<bool, CtapError> {
        if self.configuration(fs)? != Some(generation) {
            return Ok(false);
        }
        Ok(read(fs)?
            .entries
            .get(slot as usize)
            .and_then(|e| *e)
            .is_some_and(|e| e.stage == Stage::Authorized))
    }
}

#[cfg(test)]
#[path = "bio_directory_tests.rs"]
mod tests;
