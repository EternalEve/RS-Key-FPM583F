// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;
use crate::fingerprint_driver_fixture::driver;
use rsk_fido::CtapError;

#[test]
fn deleting_an_unsaved_capture_checks_absence_without_a_module_delete() {
    for samples in [0, 1, ENROLL_SAMPLES - 1] {
        let (mut driver, module, _) = driver();
        module.borrow_mut().slots[99] = true;
        if samples == 0 {
            module.borrow_mut().quality.push_back(8);
        }
        for sample in 0..samples.max(1) {
            bio_sensor::execute(
                &mut driver,
                1,
                Command::Capture {
                    slot: 7,
                    generation: 1,
                    first: sample == 0,
                    remaining: ENROLL_SAMPLES - sample,
                },
                30_000,
            )
            .unwrap();
        }
        assert!(driver.temporary.is_some());
        module.borrow_mut().delete_code = 1;
        assert_eq!(
            bio_sensor::execute(&mut driver, 1, Command::Delete { slot: 7 }, 30_000),
            Ok(bio_sensor::Result::Deleted),
            "an unsaved capture must not need a successful module delete"
        );
        assert!(driver.temporary.is_none());
        assert!(driver.ready());
        assert!(module.borrow().slots[99]);
        assert_eq!(module.borrow().calls(0x0131), 0);
        assert_eq!(module.borrow().calls(0x0132), 0);
        assert_eq!(module.borrow().calls(0x0133), MODULE_SLOTS + 1);
        assert_eq!(
            driver.start_management(
                1,
                Command::Capture {
                    slot: 7,
                    generation: 1,
                    first: false,
                    remaining: ENROLL_SAMPLES - samples,
                }
            ),
            Err(Failure::Communication)
        );
    }
}

#[test]
fn deleting_an_occupied_slot_does_not_use_a_cached_empty_inventory() {
    let (mut driver, module, _) = driver();
    bio_sensor::read_inventory(&mut driver, 1).unwrap();
    module.borrow_mut().slots[7] = true;
    module.borrow_mut().slots[99] = true;
    assert_eq!(
        bio_sensor::execute(&mut driver, 1, Command::Delete { slot: 7 }, 30_000),
        Ok(bio_sensor::Result::Deleted)
    );
    assert!(!module.borrow().slots[7]);
    assert!(module.borrow().slots[99]);
    assert_eq!(module.borrow().calls(0x0131), 1);
    assert_eq!(module.borrow().calls(0x0132), 1);
}

#[test]
fn an_occupied_slot_delete_error_is_not_reported_as_absence() {
    let (mut driver, module, _) = driver();
    module.borrow_mut().slots[7] = true;
    module.borrow_mut().delete_code = 1;
    assert_eq!(
        bio_sensor::execute(&mut driver, 1, Command::Delete { slot: 7 }, 30_000),
        Err(CtapError::Other)
    );
    assert!(module.borrow().slots[7]);
    assert_eq!(module.borrow().calls(0x0131), 1);
    assert_eq!(module.borrow().calls(0x0132), 1);
}

#[test]
fn a_failed_absence_query_never_sends_a_delete() {
    for corrupt in [false, true] {
        let (mut driver, module, _) = driver();
        module.borrow_mut().slots[7] = true;
        if corrupt {
            module.borrow_mut().corrupted = Some(0x0133);
        } else {
            module.borrow_mut().drop_command = Some(0x0133);
        }
        assert_eq!(
            bio_sensor::execute(&mut driver, 1, Command::Delete { slot: 7 }, 30_000),
            Err(CtapError::Other)
        );
        assert!(module.borrow().slots[7]);
        assert_eq!(module.borrow().calls(0x0131), 0);
    }
}

#[test]
fn an_absent_slot_still_requires_a_consistent_full_inventory() {
    let (mut driver, module, _) = driver();
    module.borrow_mut().slots[99] = true;
    module.borrow_mut().count_override = Some(2);
    assert_eq!(
        bio_sensor::execute(&mut driver, 1, Command::Delete { slot: 7 }, 30_000),
        Err(CtapError::Other)
    );
    assert_eq!(module.borrow().calls(0x0131), 0);
    assert!(module.borrow().slots[99]);
}

#[test]
fn a_slot_reappearing_during_confirmation_is_not_deleted_or_reported_absent() {
    let (mut driver, module, _) = driver();
    driver.prepare(30_000).unwrap();
    let attempt = driver
        .start_management(1, Command::Delete { slot: 7 })
        .unwrap();
    while module.borrow().calls(0x0133) == 0 {
        assert!(driver.poll_management().is_pending());
        driver.wait();
    }
    module.borrow_mut().slots[7] = true;
    for _ in 0..30_000 {
        if let Poll::Ready(observation) = driver.poll_management() {
            assert_eq!(observation.attempt, attempt);
            assert_eq!(observation.result, Err(Failure::Communication));
            driver.finish(attempt);
            assert!(module.borrow().slots[7]);
            assert_eq!(module.borrow().calls(0x0131), 0);
            return;
        }
        driver.wait();
    }
    panic!("changed inventory did not finish the bounded delete check");
}

#[test]
fn cancelling_the_target_query_cannot_issue_a_delete_when_its_reply_arrives() {
    let (mut driver, module, _) = driver();
    module.borrow_mut().slots[7] = true;
    driver.prepare(30_000).unwrap();
    let attempt = driver
        .start_management(1, Command::Delete { slot: 7 })
        .unwrap();
    while module.borrow().calls(0x0133) == 0 {
        assert!(driver.poll_management().is_pending());
        driver.wait();
    }
    driver.finish(attempt);
    for _ in 0..1_000 {
        driver.service_idle();
        if driver.ready() {
            assert!(module.borrow().slots[7]);
            assert_eq!(module.borrow().calls(0x0131), 0);
            return;
        }
        driver.wait();
    }
    panic!("cancelled target query was not drained");
}
