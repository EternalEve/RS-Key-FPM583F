// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;
use crate::fingerprint_driver_fixture::{Clock, Module, Wire};
use crate::fingerprint_service::Service;
use crate::presence::{Arbiter, Board, ButtonWait, Outcome, SCOPE_FIDO, SCOPE_NONE};
use core::cell::RefCell;
use std::rc::Rc;

type TestDriver = RefCell<Driver<Wire, Clock>>;

fn driver() -> (TestDriver, Rc<RefCell<Module>>, Clock, Rearm) {
    let module = Rc::new(RefCell::new(Module {
        controlled: true,
        powered: false,
        rx: Default::default(),
        ..Module::default()
    }));
    let clock = Clock::default();
    let mut driver = Driver::new(Wire(module.clone()), clock.clone());
    driver.initialize_touch().unwrap();
    let touch = Rearm::new();
    touch.enable();
    (RefCell::new(driver), module, clock, touch)
}

fn settle(driver: &TestDriver, touch: &Rearm) {
    for _ in 0..4_000 {
        if touch.service(&mut driver.borrow_mut()) {
            touch.observe(touch.window(), false);
            return;
        }
        driver.borrow_mut().wait();
    }
    panic!("touch maintenance did not finish");
}

#[test]
fn idle_and_held_touches_never_confirm_and_only_released_touches_wake() {
    let (driver, module, _, touch) = driver();
    touch.observe(None, true);
    for _ in 0..20 {
        touch.service(&mut driver.borrow_mut());
    }
    assert_eq!(
        module.borrow().cycles,
        1,
        "a held touch must not start maintenance"
    );
    let window = touch.open(SCOPE_FIDO).unwrap();
    touch.observe(Some(window), true);
    assert!(!touch.take(window));
    touch.observe(Some(window), false);
    settle(&driver, &touch);
    assert!(!touch.take(window), "maintenance is not consent");
    touch.observe(Some(window), false);
    touch.observe(Some(window), true);
    assert!(touch.take(window));
    touch.observe(Some(window), false);
    settle(&driver, &touch);
    touch.observe(Some(window), false);
    touch.observe(Some(window), true);
    assert!(
        !touch.take(window),
        "a spent window cannot gain a second confirmation"
    );
}

#[test]
fn edges_during_sleep_cannot_grant_consent_or_request_another_cycle() {
    let (driver, module, _, touch) = driver();
    touch.observe(None, true);
    touch.observe(None, false);
    touch.service(&mut driver.borrow_mut());
    let window = touch.open(SCOPE_FIDO).unwrap();
    touch.observe(Some(window), true);
    touch.observe(Some(window), false);
    settle(&driver, &touch);
    assert_eq!(module.borrow().cuts, 2);
    assert!(!touch.take(window));
    touch.observe(Some(window), false);
    touch.observe(Some(window), true);
    assert!(touch.take(window));
}

#[test]
fn a_high_level_after_sleep_requires_release_before_new_consent() {
    let (driver, module, _, touch) = driver();
    touch.observe(None, true);
    touch.observe(None, false);
    let window = touch.open(SCOPE_FIDO).unwrap();
    for _ in 0..4_000 {
        if touch.service(&mut driver.borrow_mut()) {
            break;
        }
        driver.borrow_mut().wait();
    }
    assert!(touch.phase() == Phase::Settling);
    touch.observe(Some(window), true);
    assert!(!touch.take(window));
    for _ in 0..20 {
        touch.service(&mut driver.borrow_mut());
    }
    assert_eq!(module.borrow().cycles, 2);
    touch.observe(Some(window), false);
    assert!(!touch.take(window));
    touch.observe(Some(window), true);
    assert!(touch.take(window));
}

#[test]
fn maintenance_power_edges_do_not_request_another_cycle() {
    let (driver, module, _, touch) = driver();
    touch.observe(None, true);
    touch.observe(None, false);
    let mut cycles = module.borrow().cycles;
    for _ in 0..2_000 {
        if touch.service(&mut driver.borrow_mut()) {
            touch.observe(None, false);
        }
        if module.borrow().cycles != cycles {
            cycles = module.borrow().cycles;
            touch.observe(None, true);
            touch.observe(None, false);
        }
        driver.borrow_mut().service_idle();
        driver.borrow_mut().wait();
    }
    assert_eq!(
        module.borrow().cycles,
        2,
        "maintenance-generated edges must not request another power cycle"
    );
    assert_eq!(driver.borrow().power_phase(), PowerPhase::Off);
}

#[test]
fn enrollment_hold_and_active_matching_defer_touch_maintenance() {
    let (driver, module, clock, touch) = driver();
    let mut service = Service::new(&driver);
    service.prepare(clock.0.get() + 30_000).unwrap();
    let attempt = service.start(1).unwrap();
    touch.observe(None, true);
    touch.observe(None, false);
    for _ in 0..20 {
        touch.service(&mut driver.borrow_mut());
    }
    assert_eq!(module.borrow().cuts, 1);
    service.finish(attempt);
    service.release(true);
    for _ in 0..20 {
        service.service_idle();
        touch.service(&mut driver.borrow_mut());
        service.wait();
    }
    assert_eq!(
        module.borrow().cuts,
        1,
        "enrollment owns the powered module"
    );
    service.release(false);
    for _ in 0..4_000 {
        service.service_idle();
        if touch.service(&mut driver.borrow_mut()) {
            touch.observe(None, false);
        }
        service.wait();
        if touch.phase() == Phase::Ready {
            break;
        }
    }
    assert!(touch.phase() == Phase::Ready);
    assert_eq!(module.borrow().cuts, 3);
}

#[test]
fn a_failed_resleep_disables_presence_without_automatic_power_retries() {
    for command in [0x0203, 0x020c] {
        let (driver, module, _, touch) = driver();
        module.borrow_mut().drop_command = Some(command);
        touch.observe(None, true);
        touch.observe(None, false);
        for _ in 0..8_000 {
            touch.service(&mut driver.borrow_mut());
            driver.borrow_mut().wait();
        }
        assert_eq!(driver.borrow().power_phase(), PowerPhase::Unavailable);
        assert!(touch.open(SCOPE_FIDO).is_none());
        touch.enable();
        assert!(
            touch.open(SCOPE_FIDO).is_none(),
            "initialization cannot revive a faulted service"
        );
        touch.observe(None, true);
        touch.observe(None, false);
        for _ in 0..20 {
            touch.service(&mut driver.borrow_mut());
        }
        assert_eq!(module.borrow().cycles, 2);
        assert_eq!(
            module.borrow().cuts,
            1,
            "an uncertain module must not be powered off"
        );
    }
}

struct PresenceBoard<'a> {
    driver: &'a TestDriver,
    touch: &'a Rearm,
    clock: Clock,
    arbiter: &'a Arbiter,
    start: u64,
    touch_at: Option<u64>,
    cancel_at: Option<u64>,
    button_at: Option<u64>,
    window: Option<u32>,
}

impl Board for PresenceBoard<'_> {
    fn pressed(&mut self) -> bool {
        self.button_at
            .is_some_and(|at| self.clock.0.get() - self.start == at)
    }
    fn now_us(&self) -> u64 {
        self.clock.0.get() * 1_000
    }
    fn block_for_ms(&mut self, ms: u64) {
        if self.touch.service(&mut self.driver.borrow_mut()) {
            self.touch.observe(self.window, false);
        }
        self.clock.0.set(self.clock.0.get() + ms);
        let elapsed = self.clock.0.get() - self.start;
        if self.cancel_at.is_some_and(|at| elapsed >= at) {
            self.arbiter.request_cancel();
        }
        if self.touch_at.is_some_and(|at| elapsed >= at) && self.touch.phase() == Phase::Ready {
            self.touch.observe(self.window, true);
            self.touch_at = None;
        }
    }
    fn begin_touch(&mut self) {
        self.window = self.touch.open(self.arbiter.wait_scope());
        self.touch.observe(self.window, false);
    }
    fn touch_confirmed(&mut self) -> bool {
        self.window.is_some_and(|w| self.touch.take(w))
    }
    fn end_touch(&mut self) {
        if let Some(w) = self.window.take() {
            self.touch.close(w);
        }
    }
}

#[test]
fn presence_wait_services_sleep_without_clearing_cancel_timeout_or_button_backup() {
    for outcome in [Outcome::Confirmed, Outcome::Cancelled, Outcome::Timeout] {
        let (driver, module, clock, touch) = driver();
        let mut service = Service::new(&driver);
        // The dispatch holds Service, while the presence backend borrows Driver.
        let dispatch = RefCell::new(&mut service);
        let _dispatch_borrow = dispatch.borrow_mut();
        touch.observe(None, true);
        touch.observe(None, false);
        let arbiter = Arbiter::new();
        arbiter.set_wait_scope(SCOPE_FIDO);
        arbiter.set_timeout_ms(10_000);
        let start = clock.0.get();
        let mut board = PresenceBoard {
            driver: &driver,
            touch: &touch,
            clock: clock.clone(),
            arbiter: &arbiter,
            start,
            window: None,
            touch_at: (outcome == Outcome::Confirmed).then_some(8_000),
            cancel_at: (outcome == Outcome::Cancelled).then_some(64),
            button_at: None,
        };
        assert_eq!(ButtonWait::new().wait(&arbiter, &mut board), outcome);
        assert!(!arbiter.pending_for(SCOPE_FIDO));
        assert!(
            clock.0.get() - start <= 10_016,
            "maintenance must not restart the request deadline"
        );
        assert!(touch.window().is_none());
        if outcome == Outcome::Confirmed {
            assert_eq!(module.borrow().cuts, 2);
        }
        if outcome != Outcome::Confirmed {
            if touch.phase() != Phase::Ready {
                settle(&driver, &touch);
            }
            board.start = clock.0.get();
            board.touch_at = Some(16);
            board.cancel_at = None;
            assert_eq!(
                ButtonWait::new().wait(&arbiter, &mut board),
                Outcome::Confirmed,
                "a fresh touch must work after cancellation or timeout"
            );
        }
        arbiter.set_wait_scope(SCOPE_NONE);
    }
    let (driver, _, clock, touch) = driver();
    touch.observe(None, true);
    let arbiter = Arbiter::new();
    arbiter.set_wait_scope(SCOPE_FIDO);
    let mut board = PresenceBoard {
        driver: &driver,
        touch: &touch,
        start: clock.0.get(),
        clock,
        arbiter: &arbiter,
        window: None,
        touch_at: None,
        cancel_at: None,
        button_at: Some(16),
    };
    assert_eq!(
        ButtonWait::new().wait(&arbiter, &mut board),
        Outcome::Confirmed
    );
}

#[test]
fn a_business_request_waits_for_sleep_then_keeps_its_own_power_hold() {
    let (driver, module, clock, touch) = driver();
    touch.observe(None, true);
    touch.observe(None, false);
    touch.service(&mut driver.borrow_mut());
    assert_eq!(module.borrow().cycles, 2);
    let mut service = Service::new(&driver);
    service.prepare(clock.0.get() + 30_000).unwrap();
    assert_eq!(
        module.borrow().cuts,
        2,
        "prepare must finish the outstanding ordinary sleep"
    );
    assert_eq!(module.borrow().cycles, 3);
    assert!(touch.service(&mut driver.borrow_mut()));
    assert!(service.ready());
    assert_eq!(driver.borrow().power_phase(), PowerPhase::Running);
    assert_eq!(
        module.borrow().cuts,
        2,
        "touch maintenance must not release the new business hold"
    );
    service.release(false);
}

#[test]
fn a_pending_cycle_never_arms_a_new_window() {
    let (_, _, _, touch) = driver();
    touch.observe(None, true);
    touch.observe(None, false);
    let window = touch.open(SCOPE_FIDO).unwrap();
    touch.observe(Some(window), false);
    touch.observe(Some(window), true);
    assert!(
        !touch.take(window),
        "touch before ordinary sleep must be refused"
    );
}

#[test]
fn cancelled_or_expired_prepare_does_not_abort_background_sleep() {
    for cancelled in [true, false] {
        let (driver, module, clock, touch) = driver();
        touch.observe(None, true);
        touch.observe(None, false);
        touch.service(&mut driver.borrow_mut());
        let mut service = Service::new(&driver);
        clock.1.set(cancelled);
        let expected = if cancelled {
            rsk_fido::CtapError::KeepAliveCancel
        } else {
            rsk_fido::CtapError::UserActionTimeout
        };
        assert_eq!(service.prepare(clock.0.get() + 1), Err(expected));
        assert_ne!(
            driver.borrow().power_phase(),
            PowerPhase::Unavailable,
            "request cancellation or expiry must not disable background maintenance"
        );
        service.release(false);
        clock.1.set(false);
        settle(&driver, &touch);
        assert_eq!(driver.borrow().power_phase(), PowerPhase::Off);
        assert_eq!(module.borrow().cycles, 2);
        assert_eq!(module.borrow().cuts, 2);
        service.prepare(clock.0.get() + 30_000).unwrap();
        assert!(service.ready());
        service.release(false);
    }
}
