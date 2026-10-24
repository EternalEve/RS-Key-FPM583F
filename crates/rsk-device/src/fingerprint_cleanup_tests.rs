// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;
use crate::fingerprint_driver_fixture::driver;
use crate::tests::{Env, SERIAL_HASH, SERIAL_ID};
use minicbor::Encoder;
use minicbor::encode::Write;
use minicbor::encode::write::Cursor;
use rsk_crypto::pinproto::{self, PinProto};
use rsk_fido::bio_directory::{self, Directory, Entry, Head, Stage};
use rsk_fido::consts::{BIO_ID_LEN, BIO_NAME_MAX, CTAP_BIO_ENROLLMENT, EF_PIN};
use rsk_fs::Fs;
use rsk_fs::storage::ram::RamStorage;
use std::cell::RefCell;

const TOKEN: [u8; 32] = [0x53; 32];

fn remove(id: &[u8; BIO_ID_LEN]) -> Vec<u8> {
    let mut params = [0; 32];
    let mut enc = Encoder::new(Cursor::new(&mut params[..]));
    enc.map(1).unwrap().u8(1).unwrap().bytes(id).unwrap();
    let len = enc.writer().position();
    let params = &params[..len];
    let mut message = vec![1, 6];
    message.extend_from_slice(params);
    let mut auth = [0; 32];
    pinproto::authenticate(PinProto::Two, &TOKEN, &message, &mut auth).unwrap();
    let mut data = [0; 128];
    data[0] = CTAP_BIO_ENROLLMENT;
    let mut enc = Encoder::new(Cursor::new(&mut data[1..]));
    enc.map(5)
        .unwrap()
        .u8(1)
        .unwrap()
        .u8(1)
        .unwrap()
        .u8(2)
        .unwrap()
        .u8(6)
        .unwrap()
        .u8(3)
        .unwrap();
    enc.writer_mut().write_all(params).unwrap();
    enc.u8(4)
        .unwrap()
        .u8(2)
        .unwrap()
        .u8(5)
        .unwrap()
        .bytes(&auth)
        .unwrap();
    let len = enc.writer().position() + 1;
    data[..len].to_vec()
}

#[test]
fn authorized_cleanup_retires_old_unsaved_records_and_preserves_other_templates() {
    for stage in [Stage::Pending, Stage::Saved, Stage::Deleting] {
        let mut fs = Fs::new(RamStorage::new());
        let dev = rsk_crypto::Device {
            serial_hash: &SERIAL_HASH,
            serial_id: &SERIAL_ID,
            otp_key: None,
        };
        rsk_fido::clientpin::store_local_pin(&dev, &mut fs, b"58273046").unwrap();
        bio_directory::write_head(
            &mut fs,
            Head {
                generation: 2,
                isolated: false,
            },
        )
        .unwrap();
        let target = Entry {
            stage,
            generation: 2,
            id: [0xA5; BIO_ID_LEN],
            name: [0; BIO_NAME_MAX],
            name_len: 0,
        };
        let other = Entry {
            stage: Stage::Authorized,
            generation: 1,
            id: [0x5A; BIO_ID_LEN],
            name: [0; BIO_NAME_MAX],
            name_len: 0,
        };
        bio_directory::write_entry(&mut fs, 0, target).unwrap();
        bio_directory::write_entry(&mut fs, 1, other).unwrap();
        let env = Env::with_storage(fs.into_storage());
        env.fs.borrow_mut().scan();
        let (sensor, module, clock) = driver();
        module.borrow_mut().slots[1] = true;
        module.borrow_mut().delete_code = 1;
        let sensor = RefCell::new(sensor);
        let bindings = RefCell::new(Directory);
        let mut ctap = env.ctap();
        ctap.set_fingerprint(&sensor, &bindings);
        let request = remove(&target.id);
        let mut pin = [0; 64];
        let pin_len = env.fs.borrow_mut().read(EF_PIN, &mut pin).unwrap();
        let before = bio_directory::read(&mut env.fs.borrow_mut()).unwrap();
        assert_eq!(
            ctap.handle_cbor(1, &request, clock.now_ms()),
            &[rsk_fido::CtapError::PinAuthInvalid.as_u8()]
        );
        assert!(module.borrow().commands.is_empty());
        assert_eq!(
            bio_directory::read(&mut env.fs.borrow_mut())
                .unwrap()
                .entries,
            before.entries
        );
        {
            let mut state = env.fido_state.borrow_mut();
            state.paut.token = TOKEN;
            state.begin_using_token(false, clock.now_ms());
            state.paut.permissions = rsk_fido::state::PERM_BE;
        }
        assert_eq!(ctap.handle_cbor(1, &request, clock.now_ms()), &[0]);
        let snapshot = bio_directory::read(&mut env.fs.borrow_mut()).unwrap();
        let retired = snapshot.entries[0].unwrap();
        assert_eq!(retired.stage, Stage::Retired);
        assert_eq!(retired.id, target.id);
        assert_eq!(snapshot.entries[1], Some(other));
        assert!(module.borrow().slots[1]);
        assert_eq!(module.borrow().calls(0x0131), 0);
        let mut after = [0; 64];
        assert_eq!(env.fs.borrow_mut().read(EF_PIN, &mut after), Some(pin_len));
        assert_eq!(after, pin);
        assert_eq!(
            ctap.handle_cbor(1, &request, clock.now_ms()),
            &[rsk_fido::CtapError::InvalidOption.as_u8()]
        );
    }
}
