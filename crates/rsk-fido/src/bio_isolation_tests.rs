// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;
use crate::bio_sensor;

fn read_seed_record<S: rsk_fs::Storage>(fs: &mut rsk_fs::Fs<S>) -> Option<Vec<u8>> {
    let mut record = [0; rsk_fs::MAX_VALUE_BYTES];
    fs.try_read_key(EF_KEY_DEV, &mut record)
        .unwrap()
        .map(|n| record[..n].to_vec())
}

#[test]
fn malformed_records_block_disabled_build_reset_and_both_seed_sources() {
    for fid in [EF_BIO_DIRECTORY, EF_BIO_RESET, EF_BIO_TEMPLATE] {
        for bytes in [&[][..], &[2][..]] {
            let mut env = Environment::provisioned(RamStorage::new());
            let seed = env.ctx().load_keydev().unwrap();
            let seed_record = read_seed_record(&mut env.fs);
            if fid == EF_BIO_DIRECTORY {
                let mut inventory = Inventory::default();
                inventory.set((bio_sensor::MODULE_SLOTS - 1) as u16, true);
                bio_directory::reconcile(&mut env.fs, inventory, None).unwrap();
                assert!(env.snapshot().head.unwrap().isolated);
            }
            env.fs.put(fid, bytes).unwrap();
            let mut env = env.reopen();
            let commands = env.module.commands.len();
            let mut ctx = env.ctx();
            ctx.fingerprint = None;
            assert!(
                ctx.load_keydev().is_none(),
                "{fid:#06x}: flash seed refused"
            );
            ctx.state.keydev_dec = Some(seed);
            assert!(ctx.load_keydev().is_none(), "{fid:#06x}: RAM seed refused");
            let mut out = [0; 64];
            crate::process_cbor(&mut ctx, &[CTAP_RESET], &mut out);
            assert_eq!(
                out[0],
                CtapError::Other.as_u8(),
                "{fid:#06x}: malformed record must refuse Reset"
            );
            let mut record = [0; 1];
            assert_eq!(ctx.fs.try_read(fid, &mut record), Ok(Some(bytes.len())));
            assert_eq!(&record[..bytes.len()], bytes);
            assert_eq!(read_seed_record(ctx.fs), seed_record);
            assert_eq!(ctx.state.keydev_dec, Some(seed));
            assert_eq!(env.module.commands.len(), commands);
        }
    }
}

#[test]
fn malformed_records_block_seed_initialization_without_replacing_surviving_state() {
    for fid in [EF_BIO_DIRECTORY, EF_BIO_RESET, EF_BIO_TEMPLATE] {
        for bytes in [&[][..], &[2][..]] {
            for existing_seed in [false, true] {
                let mut env = Environment::provisioned(RamStorage::new());
                if !existing_seed {
                    env.fs.delete_key(EF_KEY_DEV).unwrap();
                }
                let seed_record = read_seed_record(&mut env.fs);
                env.fs.put(EF_COUNTER, &17u32.to_le_bytes()).unwrap();
                env.fs.put(fid, bytes).unwrap();
                let mut env = env.reopen();
                assert_eq!(
                    crate::seed::ensure_seed(
                        &Environment::<RamStorage>::dev(),
                        &mut env.fs,
                        &mut Random(5)
                    ),
                    Err(rsk_sdk::error::Error::ExecError),
                    "{fid:#06x}, existing_seed={existing_seed}: malformed record must refuse init"
                );
                assert_eq!(read_seed_record(&mut env.fs), seed_record);
                let mut counter = [0; 4];
                assert_eq!(env.fs.try_read(EF_COUNTER, &mut counter), Ok(Some(4)));
                assert_eq!(counter, 17u32.to_le_bytes());
                assert_eq!(env.fs.try_read(fid, &mut [0; 1]), Ok(Some(bytes.len())));
            }
        }
    }
}

#[test]
fn isolated_state_still_refuses_disabled_build_reset_and_seed_reprovisioning() {
    let mut env = Environment::provisioned(RamStorage::new());
    let mut inventory = Inventory::default();
    inventory.set((bio_sensor::MODULE_SLOTS - 1) as u16, true);
    bio_directory::reconcile(&mut env.fs, inventory, None).unwrap();
    assert_eq!(bio_directory::configuration_clean(&mut env.fs), Ok(false));
    {
        let mut ctx = env.ctx();
        ctx.fingerprint = None;
        assert!(ctx.load_keydev().is_none());
        let mut out = [0; 64];
        crate::process_cbor(&mut ctx, &[CTAP_RESET], &mut out);
        assert_eq!(out[0], CtapError::NotAllowed.as_u8());
    }
    crate::seed::ensure_seed(
        &Environment::<RamStorage>::dev(),
        &mut env.fs,
        &mut Random(5),
    )
    .unwrap();
    env.fs.delete_key(EF_KEY_DEV).unwrap();
    assert!(
        crate::seed::ensure_seed(
            &Environment::<RamStorage>::dev(),
            &mut env.fs,
            &mut Random(5)
        )
        .is_err()
    );
    assert!(!env.fs.try_has_key(EF_KEY_DEV).unwrap());
}

#[test]
fn unconfigured_and_legally_cleaned_records_allow_disabled_build_seed_and_reset() {
    for retired in [false, true] {
        let mut env = Environment::provisioned(RamStorage::new());
        let seed = env.ctx().load_keydev().unwrap();
        if retired {
            let id = env.enroll();
            assert_eq!(env.exchange(6, Some(&id), None)[0], 0);
            assert!(env.module.inventory.empty());
            let snapshot = env.snapshot();
            assert!(!snapshot.head.unwrap().isolated);
            assert_eq!(snapshot.entries[0].unwrap().stage, Stage::Retired);
        }
        let mut env = env.reopen();
        assert_eq!(bio_directory::configuration_clean(&mut env.fs), Ok(true));
        crate::seed::ensure_seed(
            &Environment::<RamStorage>::dev(),
            &mut env.fs,
            &mut Random(5),
        )
        .unwrap();
        let mut ctx = env.ctx();
        ctx.fingerprint = None;
        assert_eq!(ctx.load_keydev(), Some(seed));
        let mut out = [0; 64];
        crate::process_cbor(&mut ctx, &[CTAP_RESET], &mut out);
        assert_eq!(out[0], 0);
        assert_ne!(ctx.load_keydev().unwrap(), seed);
        assert_eq!(bio_directory::configuration_clean(ctx.fs), Ok(true));
    }
}

#[test]
fn explicit_fingerprint_reset_clears_empty_records_only_after_module_clear() {
    for fid in [EF_BIO_DIRECTORY, EF_BIO_RESET, EF_BIO_TEMPLATE] {
        let mut env = Environment::provisioned(RamStorage::new());
        env.enroll();
        let seed = env.ctx().load_keydev().unwrap();
        env.fs.put(fid, &[]).unwrap();
        let mut env = env.reopen();
        let commands = env.module.commands.len();
        assert_eq!(env.reset(), 0);
        assert_eq!(
            &env.module.commands[commands..],
            &[Command::Clear, Command::Inventory]
        );
        assert!(env.module.inventory.empty());
        assert_eq!(env.fs.try_read(fid, &mut []), Ok(None));
        assert_eq!(bio_directory::configuration_clean(&mut env.fs), Ok(true));
        assert_ne!(env.ctx().load_keydev().unwrap(), seed);
    }
}
