// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;

#[test]
fn parses_raw_subcommand_parameters_without_reencoding() {
    let bytes = [0xa3, 1, 1, 2, 1, 3, 0xa1, 3, 0x19, 1, 0];
    let req = parse(&bytes).unwrap();
    assert_eq!(req.raw, &bytes[6..]);
    assert_eq!(req.timeout, 256);
}

#[path = "bio_tests_support.rs"]
mod support;
use crate::bio_directory::Stage;
use crate::bio_sensor::{Command, Feedback, Inventory};
use crate::consts::*;
use crate::fingerprint::Sensor;

#[test]
fn idle_expiry_and_new_commands_release_the_session_without_waking_the_sensor() {
    for expire in [true, false] {
        let mut env = Environment::provisioned(RamStorage::new());
        env.begin();
        assert_eq!(env.module.holds.last(), Some(&true));
        let prepares = env.module.prepares;
        if expire {
            env.module.now += crate::bio_sensor::SESSION_TIMEOUT_MS;
            crate::service_fingerprint(&mut env.state, &mut env.module);
        } else {
            let mut out = [0; MAX_MSG_SIZE as usize];
            crate::process_cbor(&mut env.ctx(), &[CTAP_SELECTION], &mut out);
            assert_eq!(out[0], 0);
        }
        assert!(env.state.enrollment.is_none());
        assert_eq!(env.module.holds.last(), Some(&false));
        assert_eq!(
            env.module.prepares, prepares,
            "session release must not request startup"
        );
        assert_eq!(env.snapshot().entries[0].unwrap().stage, Stage::Pending);
    }
}

#[test]
fn startup_and_inventory_cannot_extend_an_expiring_management_token() {
    let mut env = Environment::provisioned(RamStorage::new());
    env.module.now = crate::consts::PUAT_MAX_USAGE_PERIOD_MS - 100;
    env.state.paut.last_used_ms = env.module.now;
    env.state.paut.issued_at_ms = 0;
    env.module.startup_ms = 200;
    assert_eq!(
        env.exchange(1, None, None)[0],
        CtapError::PinAuthInvalid.as_u8()
    );
    assert!(
        env.module
            .commands
            .iter()
            .all(|command| *command == crate::bio_sensor::Command::Inventory)
    );
    assert_eq!(env.module.holds.last(), Some(&false));
    assert!(env.state.enrollment.is_none());
}

#[test]
fn a_refused_startup_does_not_spend_a_biometric_attempt() {
    let mut env = Environment::provisioned(RamStorage::new());
    env.enroll();
    env.module.startup_ms = crate::fingerprint::VERIFY_TIMEOUT_MS;
    assert_eq!(
        crate::fingerprint::verify(&mut env.ctx()),
        Err(CtapError::UserActionTimeout)
    );
    assert_eq!(
        crate::fingerprint::retries(&mut env.fs),
        Ok(Some(MAX_UV_RETRIES))
    );
}

#[test]
fn a_config_request_with_an_invalid_fingerprint_token_does_not_prepare_inventory() {
    let mut env = Environment::provisioned(RamStorage::new());
    env.enroll();
    env.state.paut.fingerprint_verified = true;
    env.state.paut.permissions = crate::state::PERM_GA;
    let prepares = env.module.prepares;
    let mut request = vec![CTAP_CONFIG, 0xa3, 1, 2, 3, 1, 4, 0x50];
    request.extend_from_slice(&[0; 16]);
    let mut out = [0; MAX_MSG_SIZE as usize];
    crate::process_cbor(&mut env.ctx(), &request, &mut out);
    assert_eq!(out[0], CtapError::PinAuthInvalid.as_u8());
    assert_eq!(
        env.module.prepares, prepares,
        "invalid authentication must be refused before module preparation"
    );
}
use rsk_fs::storage::{
    faults::{Cut, ProbeStuck},
    ram::RamStorage,
};
use support::*;

#[test]
fn final_authorization_requires_the_last_commit_at_every_cut_boundary() {
    for budget in 0..=2 {
        let (storage, medium) = Cut::new();
        let mut env = Environment::provisioned(storage);
        let id = env.begin();
        assert_eq!(env.exchange(2, Some(&id), None)[0], 0);
        medium.clear_ops();
        medium.arm(budget);
        let response = env.exchange(2, Some(&id), None);
        assert_eq!(response[0] == 0, budget == 2);
        assert!(
            env.module.inventory.contains(0),
            "the final module save really ran"
        );
        medium.arm(u32::MAX);
        let mut env = env.reopen();
        env.reconcile().unwrap();
        let entry = env.snapshot().entries[0].unwrap();
        assert_eq!(entry.id, id);
        assert_eq!(
            entry.stage == Stage::Authorized,
            budget == 2,
            "budget {budget}"
        );
        if budget < 2 {
            assert_eq!(env.configured(), Ok(None));
            assert_eq!(
                env.exchange(4, None, None)[0],
                0,
                "known saved residue must remain manageable"
            );
            assert_eq!(env.exchange(6, Some(&id), None)[0], 0);
            assert!(!env.module.inventory.contains(0));
        } else {
            assert_eq!(
                env.exchange(4, None, None)[0],
                0,
                "a lost completion response must not delete an authorized entry"
            );
        }
    }
}

#[test]
fn beginning_never_touches_the_module_before_persistent_reservation() {
    for budget in 0..=4 {
        let (storage, medium) = Cut::new();
        let mut env = Environment::provisioned(storage);
        medium.arm(budget);
        let response = env.exchange(1, None, None);
        let captures = env
            .module
            .commands
            .iter()
            .filter(|c| matches!(c, Command::Capture { .. }))
            .count();
        assert_eq!(captures, usize::from(budget == 4));
        assert_eq!(response[0] == 0, budget == 4);
        medium.arm(u32::MAX);
        let mut env = env.reopen();
        assert_eq!(env.configured(), Ok(None));
        assert!(env.module.inventory.empty());
    }
}

#[test]
fn deleting_is_disabled_before_module_writes_and_all_cuts_keep_old_id_reserved() {
    for budget in 0..=2 {
        let (storage, medium) = Cut::new();
        let mut env = Environment::provisioned(storage);
        let id = env.enroll();
        medium.clear_ops();
        medium.arm(budget);
        let before = env.module.commands.len();
        let response = env.exchange(6, Some(&id), None);
        assert_eq!(response[0] == 0, budget == 2);
        let deletes = env.module.commands[before..]
            .iter()
            .filter(|c| matches!(c, Command::Delete { .. }))
            .count();
        assert_eq!(deletes, usize::from(budget > 0));
        assert_eq!(env.module.inventory.contains(0), budget == 0);
        medium.arm(u32::MAX);
        let mut env = env.reopen();
        env.reconcile().unwrap();
        assert_eq!(
            env.snapshot().entries[0].unwrap().stage,
            match budget {
                0 => Stage::Authorized,
                1 => Stage::Deleting,
                _ => Stage::Retired,
            }
        );
        if budget == 1 {
            assert_eq!(env.configured(), Ok(None));
            assert_eq!(env.exchange(6, Some(&id), None)[0], 0);
        }
        if budget > 0 {
            let replacement = env.enroll();
            assert_ne!(replacement, id);
            assert_eq!(
                env.exchange(6, Some(&id), None)[0],
                CtapError::InvalidOption.as_u8()
            );
            assert!(env.module.inventory.contains(0));
        }
    }
}

#[test]
fn final_save_failure_unknown_result_and_stale_callback_never_authorize() {
    for mode in 0..3 {
        let mut env = Environment::provisioned(RamStorage::new());
        let id = env.begin();
        assert_eq!(env.exchange(2, Some(&id), None)[0], 0);
        env.module.fail = mode == 0;
        env.module.unknown = mode == 1;
        // A stale callback is injected after the preflight inventory, at execute's actual row.
        if mode == 2 {
            let sensor = &mut env.module;
            sensor.service_idle();
            sensor.stale = true;
            assert_eq!(
                crate::bio_sensor::execute(
                    sensor,
                    0,
                    Command::Capture {
                        slot: 0,
                        remaining: 1,
                        generation: 1,
                        first: false
                    },
                    100
                ),
                Err(CtapError::Other)
            );
        } else {
            assert_eq!(
                env.exchange(2, Some(&id), None)[0],
                CtapError::Other.as_u8()
            );
        }
        assert_eq!(env.configured(), Ok(None));
        env.module.fail = false;
        env.module.unknown = false;
        env.module.stale = false;
        let mut env = env.reopen();
        env.reconcile().unwrap();
        assert_eq!(env.configured(), Ok(None));
    }
}

#[test]
fn an_isolated_slot_never_verifies_beside_an_authorized_template() {
    for stage in [Stage::Pending, Stage::Saved, Stage::Deleting] {
        let mut env = Environment::provisioned(RamStorage::new());
        env.enroll();
        env.begin();
        let mut entry = env.snapshot().entries[1].unwrap();
        entry.stage = stage;
        bio_directory::write_entry(&mut env.fs, 1, entry).unwrap();
        env.state.end_enrollment(None);
        env.module.inventory.set(1, stage != Stage::Pending);
        assert!(env.configured().unwrap().is_some());
        env.module.match_slot = 1;
        assert_eq!(
            crate::fingerprint::verify(&mut env.ctx()),
            Err(CtapError::UvInvalid),
            "an isolated {stage:?} match must be refused while another slot is authorized"
        );
        env.module.match_slot = 0;
        assert_eq!(
            crate::fingerprint::verify(&mut env.ctx()),
            Ok(true),
            "the committed template must remain usable beside {stage:?}"
        );
    }
}

#[test]
fn token_expiry_ends_the_session_without_removing_recovery_information() {
    let mut env = Environment::provisioned(RamStorage::new());
    let id = env.begin();
    env.module.now += crate::bio_sensor::SESSION_TIMEOUT_MS + 1;
    assert_eq!(
        env.exchange(2, Some(&id), None)[0],
        CtapError::PinAuthInvalid.as_u8()
    );
    assert_eq!(env.configured(), Ok(None));
    env.authorize();
    assert_eq!(
        env.exchange(2, Some(&id), None)[0],
        CtapError::NotAllowed.as_u8()
    );
    assert_eq!(env.snapshot().entries[0].unwrap().stage, Stage::Pending);
    assert_eq!(env.exchange(6, Some(&id), None)[0], 0);
}

#[test]
fn quality_feedback_does_not_spend_uv_retries_and_variable_sample_count_is_respected() {
    let mut env = Environment::provisioned(RamStorage::new());
    env.module.samples = 2;
    let id = env.begin();
    env.module.feedback = Feedback::PoorQuality;
    assert_eq!(env.exchange(2, Some(&id), None)[0], 0);
    assert_eq!(env.state.enrollment.unwrap().remaining, 1);
    assert_eq!(
        crate::fingerprint::retries(&mut env.fs),
        Ok(Some(MAX_UV_RETRIES))
    );
    env.module.feedback = Feedback::Good;
    assert_eq!(env.exchange(2, Some(&id), None)[0], 0);
    assert!(env.configured().unwrap().is_some());
}

#[test]
fn read_errors_truncated_enumeration_and_missing_directory_are_not_empty() {
    let (storage, medium) = ProbeStuck::new();
    let mut env = Environment::provisioned(storage);
    env.enroll();
    medium.stick(Some(EF_BIO_DIRECTORY));
    assert_eq!(env.configured(), Err(CtapError::Other));
    medium.stick(None);
    medium.truncate_walk(true);
    assert_eq!(env.exchange(4, None, None)[0], CtapError::Other.as_u8());
    medium.truncate_walk(false);
    env.fs.delete(EF_BIO_DIRECTORY).unwrap();
    assert_eq!(env.configured(), Err(CtapError::Other));
}

#[test]
fn unknown_stock_is_durably_isolated_without_import_or_erase() {
    let mut env = Environment::provisioned(RamStorage::new());
    env.module.inventory.set(99, true);
    env.reconcile().unwrap();
    assert_eq!(env.configured(), Ok(None));
    assert_eq!(
        env.exchange(1, None, None)[0],
        CtapError::NotAllowed.as_u8()
    );
    assert!(env.module.commands.iter().all(|c| *c == Command::Inventory));
    let mut env = env.reopen();
    env.module.inventory = Inventory::default();
    env.reconcile().unwrap();
    assert_eq!(
        env.exchange(1, None, None)[0],
        CtapError::NotAllowed.as_u8()
    );
    assert_eq!(env.reset(), 0);
    assert!(env.module.inventory.empty());
}

#[test]
fn module_offline_keeps_pin_state_and_does_not_invent_an_empty_inventory() {
    let mut env = Environment::provisioned(RamStorage::new());
    env.enroll();
    env.module.offline = true;
    assert_eq!(env.exchange(4, None, None)[0], CtapError::Other.as_u8());
    assert!(env.fs.try_has_data(EF_PIN).unwrap());
    assert!(
        env.configured().unwrap().is_some(),
        "configuration is distinct from temporary availability"
    );
}

#[test]
fn reset_cuts_preserve_the_marker_and_block_seed_reprovisioning_until_explicit_retry() {
    let mut completed = 0;
    let mut refused = 0;
    let (storage, medium) = Cut::new();
    let mut baseline = Environment::provisioned(storage);
    baseline.enroll();
    crate::seed::ensure_seed(&Environment::<Cut>::dev(), &mut baseline.fs, &mut Random(5)).unwrap();
    medium.clear_ops();
    assert_eq!(baseline.reset(), 0);
    let mutations = medium.ops().len() as u32;
    assert!(mutations > 0);
    for budget in 0..=mutations {
        let (storage, medium) = Cut::new();
        let mut env = Environment::provisioned(storage);
        env.enroll();
        crate::seed::ensure_seed(&Environment::<Cut>::dev(), &mut env.fs, &mut Random(5)).unwrap();
        medium.clear_ops();
        medium.arm(budget);
        let status = env.reset();
        medium.arm(u32::MAX);
        let mut env = env.reopen();
        let pending = bio_directory::reset_pending(&mut env.fs).unwrap();
        if status == 0 {
            completed += 1;
            assert!(!pending);
            assert!(env.module.inventory.empty());
        } else {
            refused += 1;
            if pending {
                assert!(
                    crate::seed::ensure_seed(
                        &Environment::<Cut>::dev(),
                        &mut env.fs,
                        &mut Random(6)
                    )
                    .is_err()
                );
                assert_eq!(env.configured(), Ok(None));
                assert!(env.ctx().load_keydev().is_none());
                assert_eq!(
                    env.reset(),
                    0,
                    "explicit legal reset must complete the remaining work"
                );
            }
        }
    }
    assert!(completed > 0 && refused > 0);
}

#[test]
fn rename_failure_reopens_the_old_committed_name_without_changing_binding() {
    for budget in 0..=1 {
        let (storage, medium) = Cut::new();
        let mut env = Environment::provisioned(storage);
        let id = env.enroll();
        medium.arm(budget);
        let response = env.exchange(5, Some(&id), Some("指纹"));
        assert_eq!(response[0] == 0, budget == 1);
        medium.arm(u32::MAX);
        let mut env = env.reopen();
        let entry = env.snapshot().entries[0].unwrap();
        assert_eq!(entry.name().unwrap(), if budget == 1 { "指纹" } else { "" });
        assert!(env.configured().unwrap().is_some());
    }
}

#[test]
fn malformed_cbor_and_modified_authentication_parameters_do_not_write() {
    let mut env = Environment::provisioned(RamStorage::new());
    let cases = [
        vec![CTAP_BIO_ENROLLMENT, 0xff],
        vec![CTAP_BIO_ENROLLMENT, 0xa2, 1, 1, 2, 0x63, b'b', b'a', b'd'],
    ];
    for request in cases {
        let before = env.module.commands.len();
        let mut out = [0; 64];
        crate::process_cbor(&mut env.ctx(), &request, &mut out);
        assert_ne!(out[0], 0);
        assert_eq!(env.module.commands.len(), before);
    }
    let id = env.enroll();
    let before = env.snapshot().entries[0].unwrap();
    env.state.paut.permissions = crate::state::PERM_GA;
    assert_eq!(
        env.exchange(5, Some(&id), Some("forbidden"))[0],
        CtapError::PinAuthInvalid.as_u8()
    );
    assert_eq!(env.snapshot().entries[0], Some(before));
}

#[test]
fn fingerprint_authorization_is_revoked_while_pin_authorization_remains_available() {
    let mut env = Environment::provisioned(RamStorage::new());
    let id = env.enroll();
    env.state.paut.fingerprint_verified = true;
    assert_eq!(env.exchange(6, Some(&id), None)[0], 0);
    assert!(!env.state.paut.in_use);
    env.authorize();
    let id = env.enroll();
    assert_eq!(env.exchange(6, Some(&id), None)[0], 0);
    assert!(env.state.paut.in_use);
    assert!(bio_directory::configuration_clean(&mut env.fs).unwrap());
}

#[test]
fn deletion_and_reset_need_no_new_dynamic_slot_even_when_the_device_store_is_full() {
    let mut env = Environment::provisioned(RamStorage::new());
    crate::seed::ensure_seed(
        &Environment::<RamStorage>::dev(),
        &mut env.fs,
        &mut Random(5),
    )
    .unwrap();
    let id = env.enroll();
    for index in 0..env.fs.free_dynamic() {
        env.fs.put(0xA000 + index as u16, &[0xA5]).unwrap();
    }
    assert_eq!(env.fs.free_dynamic(), 0);
    assert_eq!(env.exchange(6, Some(&id), None)[0], 0);
    assert_eq!(env.reset(), 0);
    assert!(!bio_directory::reset_pending(&mut env.fs).unwrap());
}

#[test]
fn missing_retry_record_in_an_existing_directory_never_gets_a_fresh_budget() {
    let mut env = Environment::provisioned(RamStorage::new());
    let id = env.enroll();
    assert_eq!(env.exchange(6, Some(&id), None)[0], 0);
    env.fs.delete(EF_UV_STATE).unwrap();
    let before = env.module.commands.len();
    assert_eq!(env.exchange(1, None, None)[0], CtapError::Other.as_u8());
    assert_eq!(crate::fingerprint::retries(&mut env.fs), Ok(None));
    assert!(
        env.module.commands[before..]
            .iter()
            .all(|c| *c == Command::Inventory)
    );
}

#[test]
fn a_restarted_unfinished_capture_can_be_cleaned_by_an_authorized_new_begin() {
    let mut env = Environment::provisioned(RamStorage::new());
    let old = env.begin();
    let before = env.module.commands.len();
    let mut env = env.reopen();
    assert_eq!(
        env.module.commands.len(),
        before,
        "boot performs no module erase"
    );
    let new = env.begin();
    assert_ne!(new, old);
    assert_eq!(env.snapshot().entries[0].unwrap().id, new);
    assert_eq!(
        env.exchange(2, Some(&old), None)[0],
        CtapError::NotAllowed.as_u8()
    );
    let captures = env
        .module
        .commands
        .iter()
        .filter_map(|command| match command {
            Command::Capture {
                generation, first, ..
            } => Some((*generation, *first)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(captures, vec![(1, true), (2, true)]);
}

#[test]
fn corrupt_directory_never_reprovisions_a_seed_but_explicit_reset_can_clear_it() {
    let mut env = Environment::provisioned(RamStorage::new());
    env.enroll();
    crate::seed::ensure_seed(
        &Environment::<RamStorage>::dev(),
        &mut env.fs,
        &mut Random(4),
    )
    .unwrap();
    env.fs.put(EF_BIO_DIRECTORY, &[2; 10]).unwrap();
    assert!(
        crate::seed::ensure_seed(
            &Environment::<RamStorage>::dev(),
            &mut env.fs,
            &mut Random(5)
        )
        .is_err()
    );
    assert_eq!(env.reset(), 0);
    assert!(bio_directory::configuration_clean(&mut env.fs).unwrap());
}

struct UnknownWrite {
    inner: RamStorage,
    after: std::rc::Rc<std::cell::Cell<Option<u8>>>,
}
impl rsk_fs::Storage for UnknownWrite {
    fn read(&mut self, fid: u16, out: &mut [u8]) -> Option<usize> {
        self.inner.read(fid, out)
    }
    fn size(&mut self, fid: u16) -> Option<usize> {
        self.inner.size(fid)
    }
    fn for_each_key(&mut self, f: &mut dyn FnMut(u16)) -> bool {
        self.inner.for_each_key(f)
    }
    fn remove(&mut self, fid: u16) -> rsk_sdk::error::Result<()> {
        self.inner.remove(fid)
    }
    fn write(&mut self, fid: u16, data: &[u8]) -> rsk_sdk::error::Result<()> {
        self.inner.write(fid, data)?;
        if fid == EF_BIO_TEMPLATE
            && let Some(left) = self.after.get()
        {
            if left == 1 {
                self.after.set(None);
                return Err(rsk_sdk::error::Error::MemoryFatal);
            }
            self.after.set(Some(left - 1));
        }
        Ok(())
    }
}

#[test]
fn an_unknown_write_result_reopens_the_actual_saved_or_authorized_commit() {
    for nth in 1..=2 {
        let after = std::rc::Rc::new(std::cell::Cell::new(None));
        let mut env = Environment::provisioned(UnknownWrite {
            inner: RamStorage::new(),
            after: after.clone(),
        });
        let id = env.begin();
        assert_eq!(env.exchange(2, Some(&id), None)[0], 0);
        after.set(Some(nth));
        assert_eq!(
            env.exchange(2, Some(&id), None)[0],
            CtapError::Other.as_u8()
        );
        let mut env = env.reopen();
        env.reconcile().unwrap();
        let entry = env.snapshot().entries[0].unwrap();
        assert_eq!(entry.id, id);
        assert_eq!(
            entry.stage,
            if nth == 1 {
                Stage::Saved
            } else {
                Stage::Authorized
            }
        );
        assert_eq!(env.configured().unwrap().is_some(), nth == 2);
    }
}

#[test]
fn only_the_current_session_entry_ends_its_connection_scoped_enrollment() {
    let mut env = Environment::provisioned(RamStorage::new());
    env.state.channel = 9;
    let id = env.begin();
    env.state.end_enrollment(Some(10));
    assert!(env.state.enrollment.is_some());
    env.state.channel = CHANNEL_NONE;
    assert_eq!(env.exchange(2, Some(&id), None)[0], 0);
    env.state.end_enrollment(Some(9));
    assert!(env.state.enrollment.is_some());
    env.state.end_channel_sequences(CHANNEL_NONE);
    assert_eq!(
        env.exchange(2, Some(&id), None)[0],
        CtapError::NotAllowed.as_u8()
    );
    assert_eq!(env.snapshot().entries[0].unwrap().stage, Stage::Pending);
}
