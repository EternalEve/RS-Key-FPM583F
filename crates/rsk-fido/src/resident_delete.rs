// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Credential-first deletion shared by CTAP and the trusted display.
//! Refines `RSKeySecurityState!NoUnmanageableCredential` — SEC-FIDO-005.

use rsk_fs::{Fs, Storage};

use crate::consts::{EF_CRED, EF_RP, EF_RPNICK, MAX_RESIDENT_CREDENTIALS};
use crate::credential::{RP_PREFIX, RP_REC_MAX, bump_cred_store_state, slot_map};
use crate::error::CtapError;

pub(crate) fn delete<S: Storage>(
    fs: &mut Fs<S>,
    slot: u16,
    rp_id_hash: &[u8; 32],
) -> Result<(), CtapError> {
    let mut rp = [0u8; RP_REC_MAX];
    // Resolve the index before deleting anything, and retain the bytes so a
    // second read fault cannot strand its old count after the credential is gone.
    let found = read_rp(fs, rp_id_hash, &mut rp)?;
    bump_cred_store_state(fs).map_err(|_| CtapError::NotAllowed)?;
    fs.delete(EF_CRED + slot)
        .map_err(|_| CtapError::NotAllowed)?;
    crate::largeblobext::discard(fs, slot);
    // Credential first: a torn update may leave an orphan RP, never a live
    // credential whose RP entry was already removed.
    if let Some((fid, n)) = found {
        commit_decrement(fs, fid, &mut rp[..n])?;
    }
    Ok(())
}

pub(crate) fn decrement_rp<S: Storage>(
    fs: &mut Fs<S>,
    rp_id_hash: &[u8; 32],
) -> Result<(), CtapError> {
    let mut rp = [0u8; RP_REC_MAX];
    if let Some((fid, n)) = read_rp(fs, rp_id_hash, &mut rp)? {
        commit_decrement(fs, fid, &mut rp[..n])?;
    }
    Ok(())
}

fn read_rp<S: Storage>(
    fs: &mut Fs<S>,
    rp_id_hash: &[u8; 32],
    rp: &mut [u8; RP_REC_MAX],
) -> Result<Option<(u16, usize)>, CtapError> {
    let mut unread = false;
    let mut occupied = [false; MAX_RESIDENT_CREDENTIALS as usize];
    slot_map(fs, EF_RP, &mut occupied);
    for j in 0..MAX_RESIDENT_CREDENTIALS {
        if !occupied[j as usize] {
            continue;
        }
        let fid = EF_RP + j;
        let n = match fs.try_read(fid, rp) {
            Ok(Some(n)) => n.min(rp.len()),
            Ok(None) => continue,
            Err(_) => {
                unread = true;
                continue;
            }
        };
        if n >= RP_PREFIX && rp[1..RP_PREFIX] == *rp_id_hash {
            return Ok(Some((fid, n)));
        }
    }
    if unread {
        return Err(CtapError::Other);
    }
    Ok(None)
}

fn commit_decrement<S: Storage>(fs: &mut Fs<S>, fid: u16, rp: &mut [u8]) -> Result<(), CtapError> {
    rp[0] = rp[0].saturating_sub(1);
    if rp[0] == 0 {
        fs.delete(fid).map_err(|_| CtapError::NotAllowed)?;
        // An orphan nickname is never surfaced and is bound to its rpIdHash;
        // the index deletion is authoritative even if this cleanup fails.
        let _ = fs.delete(EF_RPNICK + (fid - EF_RP));
    } else {
        fs.put(fid, rp).map_err(|_| CtapError::NotAllowed)?;
    }
    Ok(())
}
