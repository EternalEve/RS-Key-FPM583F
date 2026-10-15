// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;
use crate::bio_directory::{self, Directory, Entry, Head, Stage};
use crate::consts::{BIO_ID_LEN, BIO_NAME_MAX, CTAP_CLIENT_PIN, EF_UV_STATE, MAX_UV_RETRIES};
use crate::fingerprint::{self, Attempt, Failure, Matching, Observation, Sensor};
use core::task::Poll;

const EXHAUSTED_UV: [u8; 3] = [1, 0, 0xff];

struct NoModuleWork;

impl Sensor for NoModuleWork {
    fn now_ms(&self) -> u64 {
        0
    }
    fn ready(&self) -> bool {
        false
    }
    fn start(&mut self, _: u32) -> Result<Attempt, Failure> {
        panic!("PIN fallback must not start matching")
    }
    fn poll(&mut self) -> Poll<Observation> {
        panic!("PIN fallback must not poll matching")
    }
    fn cancelled(&self) -> bool {
        false
    }
    fn matching(&self) -> Matching {
        Matching::NotStarted
    }
    fn wait(&mut self) {
        panic!("PIN fallback must not wait for the module")
    }
    fn finish(&mut self, _: Attempt) {
        panic!("PIN fallback must not finish a module operation")
    }
    fn service_idle(&mut self) {
        panic!("PIN fallback must not service the module")
    }
}

fn setup_fingerprint<S: rsk_fs::Storage>(
    storage: S,
    proto: PinProto,
    wire: u64,
) -> (Fs<S>, SeqRng, FidoState, Platform) {
    let mut fs = Fs::new(storage);
    let mut rng = SeqRng(1);
    ensure_seed(&dev(), &mut fs, &mut rng).unwrap();
    let mut state = FidoState::new();
    let plat = key_agreement(&mut fs, &mut rng, &mut state, proto, wire);
    let mut out = [0; 256];
    run(
        &mut fs,
        &mut rng,
        &mut state,
        &plat.set_pin_req(PIN),
        &mut out,
    )
    .unwrap();
    bio_directory::write_head(
        &mut fs,
        Head {
            generation: 1,
            isolated: false,
        },
    )
    .unwrap();
    bio_directory::write_entry(
        &mut fs,
        7,
        Entry {
            stage: Stage::Authorized,
            generation: 1,
            id: [7; BIO_ID_LEN],
            name: [0; BIO_NAME_MAX],
            name_len: 0,
        },
    )
    .unwrap();
    fs.put(EF_UV_STATE, &EXHAUSTED_UV).unwrap();
    (fs, rng, state, plat)
}

fn dispatch_fingerprint<S: rsk_fs::Storage>(
    fs: &mut Fs<S>,
    rng: &mut SeqRng,
    state: &mut FidoState,
    data: &[u8],
    out: &mut [u8],
) -> usize {
    let mut presence = crate::AlwaysConfirm;
    let mut sensor = NoModuleWork;
    let mut bindings = Directory;
    let mut ctx = Ctx {
        dev: dev(),
        fs,
        rng,
        state,
        now_ms: 0,
        presence: &mut presence,
        fingerprint: Some(fingerprint::Context {
            sensor: &mut sensor,
            bindings: &mut bindings,
        }),
    };
    let mut request = vec![CTAP_CLIENT_PIN];
    request.extend_from_slice(data);
    crate::process_cbor(&mut ctx, &request, out)
}

#[test]
fn correct_pin_remains_usable_with_missing_or_malformed_uv_state() {
    for (proto, wire) in [(PinProto::One, 1), (PinProto::Two, 2)] {
        for record in [
            None,
            Some(vec![]),
            Some(vec![1, MAX_UV_RETRIES, 0]),
            Some(vec![2, MAX_UV_RETRIES, !MAX_UV_RETRIES]),
            Some(vec![1, MAX_UV_RETRIES + 1, !(MAX_UV_RETRIES + 1)]),
            Some(vec![1, MAX_UV_RETRIES, !MAX_UV_RETRIES, 0]),
        ] {
            let (mut fs, mut rng, mut state, plat) =
                setup_fingerprint(RamStorage::new(), proto, wire);
            let request = plat.get_token_perms_req(PIN, PERM_BE as u64);
            let mut out = [0; 256];
            let n = dispatch_fingerprint(&mut fs, &mut rng, &mut state, &request, &mut out);
            assert_eq!(out[0], 0, "positive control: valid exhausted UV state");
            assert_eq!(plat.decrypt_token(&out[1..n]), state.paut.token);
            assert_eq!(fingerprint::retries(&mut fs), Ok(Some(MAX_UV_RETRIES)));
            fs.force_delete(EF_UV_STATE).unwrap();
            if let Some(record) = &record {
                fs.put(EF_UV_STATE, record).unwrap();
            }
            let requests = [
                (plat.get_token_req(PIN), PERM_MC | PERM_GA),
                (request, PERM_BE),
            ];
            for _ in 0..=MAX_PIN_RETRIES {
                for (request, permissions) in &requests {
                    let n = dispatch_fingerprint(&mut fs, &mut rng, &mut state, request, &mut out);
                    assert_eq!(out[0], 0, "correct PIN refused for UV record {record:?}");
                    assert_eq!(plat.decrypt_token(&out[1..n]), state.paut.token);
                    assert!(state.user_verified());
                    assert!(!state.paut.fingerprint_verified);
                    assert_eq!(state.paut.permissions, *permissions);
                    assert_eq!(ef_pin_retries(&mut fs), MAX_PIN_RETRIES);
                    let mut bytes = [0; 8];
                    let stored = fs.try_read(EF_UV_STATE, &mut bytes).unwrap();
                    assert_eq!(stored, record.as_ref().map(Vec::len));
                    if let Some(record) = &record {
                        assert_eq!(&bytes[..record.len()], record);
                    }
                }
            }
            let n = dispatch_fingerprint(
                &mut fs,
                &mut rng,
                &mut state,
                &plat.get_uv_token_req(PERM_GA as u64),
                &mut out,
            );
            assert_eq!(n, 1);
            assert_eq!(
                out[0],
                if record.is_none() {
                    CtapError::NotAllowed
                } else {
                    CtapError::Other
                }
                .as_u8()
            );
            let mut fs = Fs::new(fs.into_storage());
            fs.scan();
            let mut state = FidoState::new();
            let plat = key_agreement(&mut fs, &mut rng, &mut state, proto, wire);
            let n = dispatch_fingerprint(
                &mut fs,
                &mut rng,
                &mut state,
                &plat.get_token_perms_req(PIN, PERM_BE as u64),
                &mut out,
            );
            assert_eq!(out[0], 0, "PIN fallback must survive reopening storage");
            assert_eq!(plat.decrypt_token(&out[1..n]), state.paut.token);
            assert_eq!(ef_pin_retries(&mut fs), MAX_PIN_RETRIES);
        }
    }
}

#[test]
fn a_biometric_read_fault_preserves_pin_retries_but_issues_no_token() {
    for skip in 0..2 {
        let (storage, medium) = ProbeStuck::new();
        let (mut fs, mut rng, mut state, plat) = setup_fingerprint(storage, PinProto::Two, 2);
        let request = plat.get_token_perms_req(PIN, PERM_BE as u64);
        medium.stick_after(EF_UV_STATE, skip);
        let mut out = [0; 256];
        let n = dispatch_fingerprint(&mut fs, &mut rng, &mut state, &request, &mut out);
        assert_eq!(n, 1);
        assert_eq!(out[0], CtapError::Other.as_u8());
        assert!(!state.user_verified());
        assert_eq!(medium.value(EF_PIN).unwrap()[0], MAX_PIN_RETRIES);
        let expected = if skip == 0 {
            EXHAUSTED_UV
        } else {
            [1, MAX_UV_RETRIES, !MAX_UV_RETRIES]
        };
        assert_eq!(medium.value(EF_UV_STATE).as_deref(), Some(&expected[..]));
        let n = dispatch_fingerprint(&mut fs, &mut rng, &mut state, &request, &mut out);
        assert_eq!(out[0], 0);
        assert_eq!(plat.decrypt_token(&out[1..n]), state.paut.token);
        assert_eq!(fingerprint::retries(&mut fs), Ok(Some(MAX_UV_RETRIES)));
    }
}

#[test]
fn refused_pin_or_uv_writes_never_issue_a_token() {
    for writes in 0..3 {
        let (storage, medium) = Cut::new();
        let (mut fs, mut rng, mut state, plat) = setup_fingerprint(storage, PinProto::Two, 2);
        let request = plat.get_token_perms_req(PIN, PERM_BE as u64);
        medium.arm(writes);
        let mut out = [0; 256];
        let n = dispatch_fingerprint(&mut fs, &mut rng, &mut state, &request, &mut out);
        assert_eq!(n, 1);
        assert_eq!(out[0], CtapError::Other.as_u8());
        assert!(!state.user_verified());
        assert_eq!(
            medium.value(EF_PIN).unwrap()[0],
            if writes == 1 {
                MAX_PIN_RETRIES - 1
            } else {
                MAX_PIN_RETRIES
            },
            "PIN recovery must precede biometric recovery"
        );
        assert_eq!(
            medium.value(EF_UV_STATE).as_deref(),
            Some(&EXHAUSTED_UV[..])
        );
        medium.arm(u32::MAX);
        let n = dispatch_fingerprint(&mut fs, &mut rng, &mut state, &request, &mut out);
        assert_eq!(out[0], 0);
        assert_eq!(plat.decrypt_token(&out[1..n]), state.paut.token);
        assert_eq!(fingerprint::retries(&mut fs), Ok(Some(MAX_UV_RETRIES)));
    }
}

#[test]
fn malformed_biometric_state_does_not_change_wrong_pin_accounting() {
    let (mut fs, mut rng, mut state, plat) = setup_fingerprint(RamStorage::new(), PinProto::Two, 2);
    let malformed = [1, MAX_UV_RETRIES, 0];
    fs.put(EF_UV_STATE, &malformed).unwrap();
    let mut out = [0; 256];
    let n = dispatch_fingerprint(
        &mut fs,
        &mut rng,
        &mut state,
        &plat.get_token_perms_req(WRONG_PIN, PERM_BE as u64),
        &mut out,
    );
    assert_eq!(n, 1);
    assert_eq!(out[0], CtapError::PinInvalid.as_u8());
    assert!(!state.user_verified());
    assert_eq!(ef_pin_retries(&mut fs), MAX_PIN_RETRIES - 1);
    let plat = key_agreement(&mut fs, &mut rng, &mut state, PinProto::Two, 2);
    let n = dispatch_fingerprint(
        &mut fs,
        &mut rng,
        &mut state,
        &plat.get_token_perms_req(PIN, PERM_BE as u64),
        &mut out,
    );
    assert_eq!(out[0], 0);
    assert_eq!(plat.decrypt_token(&out[1..n]), state.paut.token);
    assert_eq!(ef_pin_retries(&mut fs), MAX_PIN_RETRIES);
    assert_eq!(state.new_pin_mismatches, 0);
    let mut record = [0; 3];
    assert_eq!(fs.try_read(EF_UV_STATE, &mut record), Ok(Some(3)));
    assert_eq!(record, malformed);
}

#[test]
fn changing_a_pin_preserves_malformed_uv_state_and_the_seed() {
    let (mut fs, mut rng, mut state, plat) = setup_fingerprint(RamStorage::new(), PinProto::Two, 2);
    let seed = load_keydev(&dev(), &mut fs).unwrap();
    let malformed = [2, MAX_UV_RETRIES, !MAX_UV_RETRIES];
    fs.put(EF_UV_STATE, &malformed).unwrap();
    let mut out = [0; 256];
    let n = dispatch_fingerprint(
        &mut fs,
        &mut rng,
        &mut state,
        &plat.change_pin_req(PIN, NEW_PIN),
        &mut out,
    );
    assert_eq!(n, 1);
    assert_eq!(out[0], 0);
    assert_eq!(load_keydev(&dev(), &mut fs), Some(seed));
    let mut record = [0; 3];
    assert_eq!(fs.try_read(EF_UV_STATE, &mut record), Ok(Some(3)));
    assert_eq!(record, malformed);
    let n = dispatch_fingerprint(
        &mut fs,
        &mut rng,
        &mut state,
        &plat.get_token_perms_req(NEW_PIN, PERM_BE as u64),
        &mut out,
    );
    assert_eq!(out[0], 0);
    assert_eq!(plat.decrypt_token(&out[1..n]), state.paut.token);
    assert_eq!(ef_pin_retries(&mut fs), MAX_PIN_RETRIES);
}
