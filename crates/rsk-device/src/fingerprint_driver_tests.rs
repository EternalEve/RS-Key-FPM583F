// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;
use crate::fingerprint_driver_fixture::{Clock, Wire, driver, response};

fn boot(driver: &mut Driver<Wire, Clock>) {
    for _ in 0..500 {
        driver.service_idle();
        if driver.ready() {
            return;
        }
        driver.wait();
    }
    panic!("read-only bootstrap did not finish");
}

fn management(
    driver: &mut Driver<Wire, Clock>,
    command: Command,
) -> Result<bio_sensor::Result, Failure> {
    let attempt = driver.start_management(1, command)?;
    for _ in 0..30_000 {
        if let Poll::Ready(observation) = driver.poll_management() {
            assert_eq!(observation.attempt, attempt);
            driver.finish(attempt);
            return observation.result;
        }
        driver.wait();
    }
    driver.finish(attempt);
    Err(Failure::Timeout)
}

fn capture(first: bool, remaining: u8) -> Command {
    Command::Capture {
        slot: 7,
        generation: 1,
        first,
        remaining,
    }
}

#[test]
fn six_successful_samples_not_proc_or_ack_trigger_save() {
    let (mut driver, module, _) = driver();
    boot(&mut driver);
    for remaining in (1..=6).rev() {
        assert_eq!(
            management(&mut driver, capture(remaining == 6, remaining)),
            Ok(bio_sensor::Result::Sample {
                feedback: Feedback::Good,
                remaining: remaining - 1,
                saved: remaining == 1,
            })
        );
        assert!(driver.ready());
        assert_eq!(module.borrow().calls(0x0113), usize::from(remaining == 1));
    }
    let module = module.borrow();
    assert_eq!(module.good_samples, 6);
    assert!(module.slots[7]);
    assert_eq!(module.calls(0x0111), 6);
    assert_eq!(module.calls(0x0114), 1);
}

#[test]
fn failed_quality_and_busy_queries_do_not_count_as_samples() {
    let (mut driver, module, _) = driver();
    boot(&mut driver);
    module.borrow_mut().quality.push_back(0x0e);
    module.borrow_mut().busy_queries = 2;
    assert_eq!(
        management(&mut driver, capture(true, 6)),
        Ok(bio_sensor::Result::Sample {
            feedback: Feedback::PoorQuality,
            remaining: 6,
            saved: false,
        })
    );
    assert_eq!(module.borrow().good_samples, 0);
    for remaining in (1..=6).rev() {
        management(&mut driver, capture(false, remaining)).unwrap();
    }
    assert_eq!(module.borrow().calls(0x0111), 7);
    assert_eq!(module.borrow().calls(0x0113), 1);
}

#[test]
fn continuation_requires_the_same_slot_generation_and_remaining_count() {
    let (mut driver, module, _) = driver();
    boot(&mut driver);
    management(&mut driver, capture(true, 6)).unwrap();
    for command in [
        capture(false, 6),
        Command::Capture {
            slot: 8,
            generation: 1,
            first: false,
            remaining: 5,
        },
        Command::Capture {
            slot: 7,
            generation: 2,
            first: false,
            remaining: 5,
        },
    ] {
        assert_eq!(
            driver.start_management(2, command),
            Err(Failure::Communication)
        );
    }
    assert_eq!(module.borrow().calls(0x0111), 1);
    management(&mut driver, capture(false, 5)).unwrap();
}

#[test]
fn inventory_scans_every_slot_and_checks_count_before_and_after() {
    let (mut driver, module, _) = driver();
    module.borrow_mut().slots[99] = true;
    module.borrow_mut().slots[0] = true;
    boot(&mut driver);
    let bio_sensor::Result::Inventory(inventory) =
        management(&mut driver, Command::Inventory).unwrap()
    else {
        panic!()
    };
    assert!(inventory.contains(99) && inventory.contains(0));
    assert_eq!(module.borrow().calls(0x0133), 100);
    module.borrow_mut().count_override = Some(1);
    assert_eq!(
        management(&mut driver, Command::Inventory),
        Err(Failure::Communication)
    );
}

#[test]
fn delete_is_targeted_and_clear_covers_unbound_slots() {
    let (mut driver, module, _) = driver();
    module.borrow_mut().slots.fill(true);
    boot(&mut driver);
    assert_eq!(
        management(&mut driver, Command::Delete { slot: 7 }),
        Ok(bio_sensor::Result::Deleted)
    );
    assert!(!module.borrow().slots[7]);
    assert!(module.borrow().slots[99]);
    assert_eq!(
        management(&mut driver, Command::Clear),
        Ok(bio_sensor::Result::Cleared)
    );
    assert!(!module.borrow().slots.iter().any(|s| *s));
    assert_eq!(module.borrow().calls(0x0131), 2);
}

#[test]
fn lost_save_response_is_never_retried_or_reported_as_saved() {
    let (mut driver, module, _) = driver();
    boot(&mut driver);
    for remaining in (2..=6).rev() {
        management(&mut driver, capture(remaining == 6, remaining)).unwrap();
    }
    module.borrow_mut().drop_command = Some(0x0114);
    assert_eq!(
        management(&mut driver, capture(false, 1)),
        Err(Failure::Communication)
    );
    assert!(module.borrow().slots[7]);
    assert_eq!(module.borrow().calls(0x0113), 1);
    assert!(
        !driver.ready(),
        "unknown write result must block the next operation"
    );
    assert_eq!(driver.start(2), Err(Failure::Unavailable));
}

#[test]
fn a_cancelled_match_is_drained_through_old_reply_cancel_and_terminal_query() {
    let (mut driver, module, _) = driver();
    boot(&mut driver);
    let attempt = driver.start(1).unwrap();
    while module.borrow().calls(0x0121) == 0 {
        let _ = driver.poll();
        driver.wait();
    }
    driver.finish(attempt);
    assert!(!driver.ready());
    assert_eq!(driver.start(2), Err(Failure::Unavailable));
    for _ in 0..1_000 {
        driver.service_idle();
        if driver.ready() {
            break;
        }
        driver.wait();
    }
    assert!(driver.ready());
    assert_eq!(module.borrow().calls(0x0115), 1);
    assert_eq!(module.borrow().calls(0x0122), 1);
    let next = driver.start(2).unwrap();
    assert_ne!(attempt.id, next.id);
    for _ in 0..1_000 {
        if let Poll::Ready(observation) = driver.poll() {
            assert_eq!(observation.attempt, next);
            assert!(observation.result.is_ok());
            driver.finish(next);
            return;
        }
        driver.wait();
    }
    panic!("next match did not finish");
}

#[test]
fn timeout_and_late_same_command_response_cannot_approve_the_next_request() {
    let (mut driver, module, _) = driver();
    boot(&mut driver);
    module.borrow_mut().drop_command = Some(0x0122);
    let attempt = driver.start(1).unwrap();
    for _ in 0..3_000 {
        if let Poll::Ready(observation) = driver.poll() {
            assert_eq!(observation.result, Err(Failure::Communication));
            driver.finish(attempt);
            break;
        }
        driver.wait();
    }
    module
        .borrow_mut()
        .rx
        .extend(response(0x0122, 0, &[0, 1, 0, 1, 0, 7]));
    for _ in 0..3_000 {
        driver.service_idle();
        driver.wait();
    }
    assert!(!driver.ready());
    assert_eq!(driver.start(2), Err(Failure::Unavailable));
    assert_eq!(module.borrow().calls(0x0121), 1);
    assert!(driver.diagnostic().unwrap().progress.wire_complete);
}

#[test]
fn duplicate_corrupt_and_unsolicited_responses_never_produce_a_match() {
    for duplicate in [true, false] {
        let (mut driver, module, _) = driver();
        boot(&mut driver);
        if duplicate {
            module.borrow_mut().duplicate_command = Some(0x0122);
        } else {
            module.borrow_mut().corrupted = Some(0x0122);
        }
        let attempt = driver.start(1).unwrap();
        for _ in 0..1_000 {
            if let Poll::Ready(observation) = driver.poll() {
                assert_eq!(observation.result, Err(Failure::Communication));
                driver.finish(attempt);
                break;
            }
            driver.wait();
        }
        assert!(!driver.ready());
    }
    let (mut driver, module, _) = driver();
    boot(&mut driver);
    module.borrow_mut().rx.push_back(0x55);
    driver.service_idle();
    assert!(
        !driver.ready(),
        "unexpected restart must not keep old authorization inputs usable"
    );
}

#[test]
fn boot_without_55_works_only_after_a_complete_read_only_reply() {
    let (mut driver, module, _) = driver();
    module.borrow_mut().rx.clear();
    boot(&mut driver);
    assert_eq!(module.borrow().commands.len(), 1);
    assert_eq!(module.borrow().commands[0].0, 0x0203);
    let (mut unavailable, module, _) = crate::fingerprint_driver_fixture::driver();
    module.borrow_mut().drop_command = Some(0x0203);
    for _ in 0..4_000 {
        unavailable.service_idle();
        unavailable.wait();
    }
    assert!(!unavailable.ready());
    assert_eq!(module.borrow().calls(0x0203), 1);
}

#[test]
fn no_lift_is_bounded_and_never_starts_another_capture() {
    let (mut driver, module, _) = driver();
    boot(&mut driver);
    management(&mut driver, capture(true, 6)).unwrap();
    module.borrow_mut().finger_present = true;
    let attempt = driver.start_management(1, capture(false, 5)).unwrap();
    for _ in 0..100 {
        let _ = driver.poll_management();
        driver.wait();
    }
    driver.finish(attempt);
    assert_eq!(module.borrow().calls(0x0111), 1);
    assert!(driver.ready());
}

#[test]
fn continuous_contact_cannot_start_another_matching_operation() {
    let (mut driver, module, clock) = driver();
    boot(&mut driver);
    module.borrow_mut().finger_present = true;
    let attempt = driver.start(1).unwrap();
    for _ in 0..1_000 {
        let _ = driver.poll();
        driver.wait();
    }
    assert_eq!(module.borrow().calls(0x0121), 0);
    assert_eq!(driver.matching(), Matching::NotStarted);
    clock.0.set(attempt.deadline_ms);
    assert_eq!(
        driver.poll().map(|o| o.result),
        Poll::Ready(Err(Failure::Timeout))
    );
    driver.finish(attempt);
    assert!(!driver.ready());
}

#[test]
fn a_known_save_failure_does_not_release_an_unsaved_sample_as_complete() {
    let (mut driver, module, _) = driver();
    boot(&mut driver);
    for remaining in (2..=6).rev() {
        management(&mut driver, capture(remaining == 6, remaining)).unwrap();
    }
    module.borrow_mut().save_code = 0x0c;
    assert_eq!(
        management(&mut driver, capture(false, 1)),
        Err(Failure::Communication)
    );
    assert_eq!(module.borrow().calls(0x0113), 1);
    assert!(!module.borrow().slots[7]);
}

#[test]
fn uncertain_delete_never_reissues_a_write_or_deletes_another_slot() {
    let (mut driver, module, _) = driver();
    boot(&mut driver);
    module.borrow_mut().slots[7] = true;
    module.borrow_mut().slots[99] = true;
    module.borrow_mut().drop_command = Some(0x0132);
    assert_eq!(
        management(&mut driver, Command::Delete { slot: 7 }),
        Err(Failure::Communication)
    );
    assert_eq!(module.borrow().calls(0x0131), 1);
    assert!(!module.borrow().slots[7]);
    assert!(module.borrow().slots[99]);
    assert!(!driver.ready());
}

#[test]
fn losing_a_cancel_ack_keeps_the_sensor_unavailable() {
    let (mut driver, module, _) = driver();
    boot(&mut driver);
    let attempt = driver.start(1).unwrap();
    for _ in 0..1_000 {
        let _ = driver.poll();
        if module.borrow().calls(0x0121) > 0 {
            break;
        }
        driver.wait();
    }
    module.borrow_mut().drop_command = Some(0x0115);
    driver.finish(attempt);
    for _ in 0..2_500 {
        driver.service_idle();
        driver.wait();
    }
    assert_eq!(driver.start(2), Err(Failure::Unavailable));
    assert_eq!(module.borrow().calls(0x0115), 1);
}
