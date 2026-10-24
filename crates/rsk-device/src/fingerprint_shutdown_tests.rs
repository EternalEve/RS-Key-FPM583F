// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;
use crate::fingerprint_driver_fixture::{Clock, Module, Wire};
use std::cell::RefCell;
use std::rc::Rc;

fn driver() -> (Driver<Wire, Clock>, Rc<RefCell<Module>>, Clock) {
    let module = Rc::new(RefCell::new(Module {
        controlled: true,
        powered: false,
        rx: Default::default(),
        ..Module::default()
    }));
    let clock = Clock::default();
    let mut driver = Driver::new(Wire(module.clone()), clock.clone());
    driver.prepare(30_000).unwrap();
    driver.release(false);
    (driver, module, clock)
}

fn advance(driver: &mut Driver<Wire, Clock>, milliseconds: u64) {
    for _ in 0..milliseconds {
        driver.service_idle();
        driver.wait();
    }
}

#[test]
fn successful_sleep_waits_for_touchout_low_before_cutting_power() {
    let (mut driver, module, _) = driver();
    module.borrow_mut().touch_high = true;
    advance(&mut driver, 500);
    assert_eq!(module.borrow().calls(0x020c), 1);
    assert!(module.borrow().rx.is_empty(), "sleep ACK was consumed");
    assert_eq!(module.borrow().cuts, 0, "high TOUCHOUT forbids a power cut");
    assert_eq!(driver.power_phase(), Phase::Stopping);
    module.borrow_mut().touch_high = false;
    advance(&mut driver, 10);
    assert_eq!(driver.power_phase(), Phase::Off);
    assert_eq!(module.borrow().cuts, 1);
    assert_eq!(module.borrow().calls(0x020c), 1);
    assert_eq!(module.borrow().commands.len(), 3);
}

#[test]
fn a_three_second_touch_pulse_has_a_separate_wait_after_sleep_ack() {
    let (mut driver, module, _) = driver();
    module.borrow_mut().touch_high = true;
    advance(&mut driver, 3_200);
    assert_eq!(driver.power_phase(), Phase::Stopping);
    assert_eq!(module.borrow().cuts, 0);
    module.borrow_mut().touch_high = false;
    advance(&mut driver, 1);
    assert_eq!(driver.power_phase(), Phase::Off);
    assert_eq!(module.borrow().calls(0x020c), 1);
    assert_eq!(module.borrow().cuts, 1);
}

#[test]
fn a_low_touchout_does_not_sleep_while_the_module_reports_a_finger() {
    let (mut driver, module, _) = driver();
    module.borrow_mut().finger_present = true;
    advance(&mut driver, 1_000);
    assert_eq!(module.borrow().calls(0x020c), 0);
    assert_eq!(module.borrow().cuts, 0);
    assert!(module.borrow().calls(0x0135) > 0);
    module.borrow_mut().finger_present = false;
    advance(&mut driver, 500);
    assert_eq!(driver.power_phase(), Phase::Off);
    assert_eq!(module.borrow().calls(0x020c), 1);
    assert_eq!(module.borrow().cuts, 1);
}

#[test]
fn an_unknown_finger_query_cannot_be_treated_as_absence() {
    for corrupt in [false, true] {
        let (mut driver, module, _) = driver();
        if corrupt {
            module.borrow_mut().corrupted = Some(0x0135);
        } else {
            module.borrow_mut().drop_command = Some(0x0135);
        }
        advance(&mut driver, SHUTDOWN_TIMEOUT_MS + 1);
        assert_eq!(driver.power_phase(), Phase::Unavailable);
        assert_eq!(module.borrow().calls(0x0135), 1);
        assert_eq!(module.borrow().calls(0x020c), 0);
        assert_eq!(module.borrow().cuts, 0);
    }
}

#[test]
fn waiting_for_module_absence_preserves_request_deadlines_and_cancellation() {
    let (mut driver, module, clock) = driver();
    module.borrow_mut().finger_present = true;
    advance(&mut driver, 500);
    let started = driver.now_ms();
    assert_eq!(
        driver.prepare(started + 100),
        Err(rsk_fido::CtapError::UserActionTimeout)
    );
    assert_eq!(driver.now_ms(), started + 100);
    driver.release(false);
    clock.1.set(true);
    assert_eq!(
        driver.prepare(driver.now_ms() + 100),
        Err(rsk_fido::CtapError::KeepAliveCancel)
    );
    driver.release(false);
    assert_eq!(driver.power_phase(), Phase::Stopping);
    assert_eq!(module.borrow().calls(0x0121), 0);
    assert_eq!(module.borrow().calls(0x020c), 0);
    clock.1.set(false);
    module.borrow_mut().finger_present = false;
    advance(&mut driver, 500);
    assert_eq!(driver.power_phase(), Phase::Off);
}

#[test]
fn stuck_high_times_out_without_a_power_cut_or_automatic_retry() {
    let (mut driver, module, _) = driver();
    module.borrow_mut().touch_high = true;
    advance(&mut driver, SHUTDOWN_TIMEOUT_MS + 1);
    assert_eq!(driver.power_phase(), Phase::Unavailable);
    assert_eq!(module.borrow().cuts, 0);
    assert!(module.borrow().powered);
    module.borrow_mut().touch_high = false;
    advance(&mut driver, SHUTDOWN_TIMEOUT_MS);
    assert_eq!(driver.power_phase(), Phase::Unavailable);
    assert_eq!(module.borrow().calls(0x020c), 1);
    assert_eq!(module.borrow().cycles, 1);
    assert_eq!(module.borrow().cuts, 0);
    assert_eq!(
        driver.prepare(driver.now_ms() + 30_000),
        Err(rsk_fido::CtapError::Other)
    );
}

#[test]
fn touchout_is_checked_after_the_ack_instead_of_before_sleep() {
    let (mut driver, module, _) = driver();
    while !matches!(driver.step, Step::PowerOff) {
        driver.service_idle();
        driver.wait();
        assert!(driver.now_ms() < 1_000);
    }
    assert!(!module.borrow().touch_high);
    module.borrow_mut().touch_high = true;
    advance(&mut driver, 500);
    assert_eq!(module.borrow().cuts, 0);
    module.borrow_mut().touch_high = false;
    advance(&mut driver, 1);
    assert_eq!(module.borrow().cuts, 1);
}

#[test]
fn a_low_level_on_the_shutdown_deadline_cannot_cut_power() {
    for on_deadline in [false, true] {
        let (mut driver, module, clock) = driver();
        module.borrow_mut().touch_high = true;
        advance(&mut driver, 500);
        let deadline = driver.power.deadline_ms;
        clock.0.set(deadline - u64::from(!on_deadline));
        module.borrow_mut().touch_high = false;
        driver.service_idle();
        assert_eq!(module.borrow().cuts, usize::from(!on_deadline));
        assert_eq!(
            driver.power_phase(),
            if on_deadline {
                Phase::Unavailable
            } else {
                Phase::Off
            }
        );
    }
}

#[test]
fn missing_or_invalid_sleep_ack_cannot_enter_the_power_off_wait() {
    for corrupt in [false, true] {
        let (mut driver, module, _) = driver();
        if corrupt {
            module.borrow_mut().corrupted = Some(0x020c);
        } else {
            module.borrow_mut().drop_command = Some(0x020c);
        }
        advance(&mut driver, SHUTDOWN_TIMEOUT_MS + 1);
        assert_eq!(driver.power_phase(), Phase::Unavailable);
        assert!(!matches!(driver.step, Step::PowerOff));
        assert_eq!(module.borrow().cuts, 0);
        assert_eq!(module.borrow().calls(0x020c), 1);
    }
}
