// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;
use rsk_fs::storage::ram::RamStorage;

#[test]
fn record_round_trip_and_unknown_version_are_closed() {
    let mut fs = Fs::new(RamStorage::new());
    write_head(
        &mut fs,
        Head {
            generation: 1,
            isolated: false,
        },
    )
    .unwrap();
    let entry = Entry {
        stage: Stage::Authorized,
        generation: 1,
        id: [7; BIO_ID_LEN],
        name: [0; BIO_NAME_MAX],
        name_len: 0,
    };
    write_entry(&mut fs, 0, entry).unwrap();
    assert_eq!(read(&mut fs).unwrap().entries[0], Some(entry));
    assert_eq!(Directory.configuration(&mut fs), Ok(Some(1)));
    fs.put(EF_BIO_DIRECTORY, &[2; HEAD_LEN]).unwrap();
    assert!(read(&mut fs).is_err());
    assert!(Directory.configuration(&mut fs).is_err());
}

#[test]
fn missing_slot_is_durably_disabled_and_never_revives() {
    let mut fs = Fs::new(RamStorage::new());
    write_head(
        &mut fs,
        Head {
            generation: 1,
            isolated: false,
        },
    )
    .unwrap();
    write_entry(
        &mut fs,
        0,
        Entry {
            stage: Stage::Authorized,
            generation: 1,
            id: [7; BIO_ID_LEN],
            name: [0; BIO_NAME_MAX],
            name_len: 0,
        },
    )
    .unwrap();
    reconcile(&mut fs, Inventory::default(), None).unwrap();
    let mut replaced = Inventory::default();
    replaced.set(0, true);
    reconcile(&mut fs, replaced, None).unwrap();
    assert_eq!(Directory.authorizes(&mut fs, 0, 1), Ok(false));
    assert_eq!(
        read(&mut fs).unwrap().entries[0].unwrap().stage,
        Stage::Deleting
    );
}

#[test]
fn empty_records_are_not_evidence_of_a_clean_configuration() {
    for fid in core::iter::once(EF_BIO_DIRECTORY)
        .chain(core::iter::once(EF_BIO_RESET))
        .chain(EF_BIO_TEMPLATE..EF_BIO_TEMPLATE + MODULE_SLOTS as u16)
    {
        for head in [false, true] {
            let mut fs = Fs::new(RamStorage::new());
            if head {
                write_head(
                    &mut fs,
                    Head {
                        generation: 0,
                        isolated: false,
                    },
                )
                .unwrap();
            }
            assert_eq!(configuration_clean(&mut fs), Ok(true));
            fs.put(fid, &[]).unwrap();
            assert_eq!(
                configuration_clean(&mut fs),
                Err(CtapError::Other),
                "empty record {fid:#06x}, head={head} must be refused"
            );
            let mut fs = Fs::new(fs.into_storage());
            fs.scan();
            assert_eq!(configuration_clean(&mut fs), Err(CtapError::Other));
            assert_eq!(fs.try_read(fid, &mut []), Ok(Some(0)));
        }
    }
}

#[test]
fn configuration_probes_preserve_storage_errors_and_recover_after_the_fault() {
    use rsk_fs::storage::faults::ProbeStuck;

    for fid in [EF_BIO_DIRECTORY, EF_BIO_RESET, EF_BIO_TEMPLATE] {
        let (storage, medium) = ProbeStuck::new();
        let mut fs = Fs::new(storage);
        medium.stick(Some(fid));
        assert_eq!(configuration_clean(&mut fs), Err(CtapError::Other));
        medium.stick(None);
        assert_eq!(configuration_clean(&mut fs), Ok(true));
    }
}
