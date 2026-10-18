// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;
use crate::state::PERM_CM;
use crate::{AlwaysConfirm, Ctx, FidoState, Rng};
use rsk_fs::storage::faults::{ProbeMedium, ProbeStuck};

const SEED: [u8; 32] = [0x42; 32];
const TOKEN: [u8; 32] = [0x99; 32];

fn dev() -> Device<'static> {
    Device {
        serial_hash: &[0xAB; 32],
        serial_id: &[1, 2, 3, 4, 5, 6, 7, 8],
        otp_key: None,
    }
}

fn store(fs: &mut Fs<ProbeStuck>, user: &[u8], iv: u8) -> Result<[u8; CRED_RESIDENT_LEN]> {
    let input = CredInput {
        rp_id: "example.com",
        user_id: user,
        user_name: "account",
        user_display_name: "Account",
        use_sign_count: true,
        rk: true,
        created_ms: 1,
        alg: ALG_ES256,
        curve: CURVE_P256 as i64,
        ext: CredExt::default(),
    };
    let mut boxed = [0u8; CRED_BOX_MAX];
    let hash = sha256(input.rp_id.as_bytes());
    let n = credential_create(&SEED, &dev(), &input, &hash, &[iv; IV_LEN], &mut boxed)?;
    credential_store(
        &SEED,
        &dev(),
        fs,
        &boxed[..n],
        &hash,
        input.rp_id,
        user,
        &[],
    )?;
    Ok(derive_resident(&boxed[..n], &dev()))
}

fn reopen(fs: Fs<ProbeStuck>) -> Fs<ProbeStuck> {
    let mut fs = Fs::new(fs.into_storage());
    fs.scan();
    fs
}

fn credentials(medium: &ProbeMedium) -> Vec<u16> {
    (0..MAX_RESIDENT_CREDENTIALS)
        .filter(|slot| medium.value(EF_CRED + slot).is_some())
        .collect()
}

#[test]
fn a_faulted_dedup_probe_refuses_without_duplicating_the_account() {
    for once in [false, true] {
        let (backend, medium) = ProbeStuck::new();
        let mut fs = Fs::new(backend);
        fs.scan();
        store(&mut fs, b"alice", 1).unwrap();
        let original = medium.value(EF_CRED).unwrap();
        let rp = medium.value(EF_RP).unwrap();
        let mut fs = reopen(fs);
        let generation = fs.write_gen();
        if once {
            medium.stick_once(EF_CRED);
        } else {
            medium.stick(Some(EF_CRED));
        }
        let result = store(&mut fs, b"alice", 2);
        assert_eq!(
            fs.write_gen(),
            generation,
            "a refused dedup changed the store"
        );
        medium.stick(None);
        let mut fs = reopen(fs);
        assert_eq!(
            credentials(&medium),
            vec![0],
            "a read fault created a duplicate account"
        );
        assert_eq!(medium.value(EF_CRED).unwrap(), original);
        assert_eq!(medium.value(EF_RP).unwrap(), rp);
        assert_eq!(result, Err(Error::MemoryFatal));

        store(&mut fs, b"alice", 3).unwrap();
        assert_eq!(credentials(&medium), vec![0]);
        assert_ne!(medium.value(EF_CRED).unwrap(), original);
        assert_eq!(medium.value(EF_RP).unwrap()[0], 1);
    }
}

#[test]
fn a_faulted_other_slot_does_not_prevent_replacing_a_readable_account() {
    let (backend, medium) = ProbeStuck::new();
    let mut fs = Fs::new(backend);
    fs.scan();
    store(&mut fs, b"alice", 1).unwrap();
    store(&mut fs, b"bob", 2).unwrap();
    let alice = medium.value(EF_CRED).unwrap();
    let bob = medium.value(EF_CRED + 1).unwrap();
    medium.stick(Some(EF_CRED));
    store(&mut fs, b"bob", 3).unwrap();
    assert_eq!(credentials(&medium), vec![0, 1]);
    assert_eq!(medium.value(EF_CRED).unwrap(), alice);
    assert_ne!(medium.value(EF_CRED + 1).unwrap(), bob);
    assert_eq!(medium.value(EF_RP).unwrap()[0], 2);
}

struct UnusedRng;

impl Rng for UnusedRng {
    fn fill(&mut self, _: &mut [u8]) {
        panic!("credential deletion must not draw randomness");
    }
}

fn host_delete(fs: &mut Fs<ProbeStuck>, id: &[u8]) -> crate::error::CtapResult {
    use rsk_crypto::pinproto::{self, PinProto};
    let mut params = [0u8; 128];
    let mut enc = minicbor::Encoder::new(minicbor::encode::write::Cursor::new(&mut params[..]));
    enc.map(1).unwrap().u8(2).unwrap().map(2).unwrap();
    enc.str("id").unwrap().bytes(id).unwrap();
    enc.str("type").unwrap().str("public-key").unwrap();
    let n = enc.writer().position();
    let mut payload = vec![crate::consts::CM_DELETE_CREDENTIAL as u8];
    payload.extend_from_slice(&params[..n]);
    let mut mac = [0u8; 32];
    let m = pinproto::authenticate(PinProto::Two, &TOKEN, &payload, &mut mac).unwrap();
    let mut request = vec![0xA4, 1, crate::consts::CM_DELETE_CREDENTIAL as u8, 2];
    request.extend_from_slice(&params[..n]);
    request.extend_from_slice(&[3, 2, 4, 0x58, m as u8]);
    request.extend_from_slice(&mac[..m]);
    let mut state = FidoState::new();
    state.paut.token = TOKEN;
    state.paut.permissions = PERM_CM;
    state.begin_using_token(false, 0);
    let mut ctx = Ctx {
        fingerprint: None,
        presence: &mut AlwaysConfirm,
        dev: dev(),
        fs,
        rng: &mut UnusedRng,
        state: &mut state,
        now_ms: 10,
    };
    crate::credmgmt::cred_mgmt(&mut ctx, &request, &mut [0u8; 256])
}

#[test]
fn a_faulted_rp_probe_refuses_both_delete_entries_before_mutating() {
    for host in [false, true] {
        for count in [1, 2] {
            for once in [false, true] {
                let (backend, medium) = ProbeStuck::new();
                let mut fs = Fs::new(backend);
                fs.scan();
                let id = store(&mut fs, b"alice", 1).unwrap();
                if count == 2 {
                    store(&mut fs, b"bob", 2).unwrap();
                }
                let original = medium.value(EF_CRED).unwrap();
                let rp = medium.value(EF_RP).unwrap();
                let mut fs = reopen(fs);
                if once {
                    medium.stick_once(EF_RP);
                } else {
                    medium.stick(Some(EF_RP));
                }
                let removed = if host {
                    let result = host_delete(&mut fs, &id);
                    assert_eq!(result, Err(crate::CtapError::Other));
                    result.is_ok()
                } else {
                    crate::passkeys::delete_cred(&mut fs, EF_CRED)
                };
                assert!(
                    !removed,
                    "a faulted RP probe was acknowledged as a successful deletion"
                );
                assert_eq!(fs.write_gen(), 0, "a refused preflight changed the store");
                medium.stick(None);
                let mut fs = reopen(fs);
                assert_eq!(medium.value(EF_CRED).unwrap(), original);
                assert_eq!(medium.value(EF_RP).unwrap(), rp);
                assert_eq!(credentials(&medium).len(), count);

                if host {
                    host_delete(&mut fs, &id).unwrap();
                } else {
                    assert!(crate::passkeys::delete_cred(&mut fs, EF_CRED));
                }
                assert!(medium.value(EF_CRED).is_none());
                if count == 1 {
                    assert!(medium.value(EF_RP).is_none());
                } else {
                    assert_eq!(medium.value(EF_RP).unwrap()[0], 1);
                }
            }
        }
    }
}

#[test]
fn a_refused_rp_removal_is_not_reported_as_a_successful_delete() {
    for host in [false, true] {
        let (backend, medium) = ProbeStuck::new();
        let mut fs = Fs::new(backend);
        fs.scan();
        let id = store(&mut fs, b"alice", 1).unwrap();
        medium.refuse_remove(Some(EF_RP));
        if host {
            assert_eq!(host_delete(&mut fs, &id), Err(crate::CtapError::NotAllowed));
        } else {
            assert!(!crate::passkeys::delete_cred(&mut fs, EF_CRED));
        }
        assert!(medium.value(EF_CRED).is_none());
        assert!(medium.value(EF_RP).is_some());
    }
}

#[test]
fn an_unread_other_rp_does_not_prevent_deleting_a_readable_rp() {
    for host in [false, true] {
        let (backend, medium) = ProbeStuck::new();
        let mut fs = Fs::new(backend);
        fs.scan();
        let id = store(&mut fs, b"alice", 1).unwrap();
        let rp = medium.value(EF_RP).unwrap();
        fs.put(EF_RP + 1, &rp).unwrap();
        let mut other = rp;
        other[1..RP_PREFIX].copy_from_slice(&sha256(b"other.example"));
        fs.put(EF_RP, &other).unwrap();
        medium.stick(Some(EF_RP));
        if host {
            host_delete(&mut fs, &id).unwrap();
        } else {
            assert!(crate::passkeys::delete_cred(&mut fs, EF_CRED));
        }
        assert!(medium.value(EF_CRED).is_none());
        assert!(medium.value(EF_RP + 1).is_none());
        assert_eq!(medium.value(EF_RP).unwrap(), other);
    }
}
