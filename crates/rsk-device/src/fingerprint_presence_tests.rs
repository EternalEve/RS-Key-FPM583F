// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;
use crate::fingerprint_driver_fixture::{Module, Wire};
use crate::presence::{Arbiter, SCOPE_CCID, SCOPE_FIDO, SCOPE_NONE};
use rsk_fido::consts::CHANNEL_NONE;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Clone, Default)]
struct BoardClock {
    now: Rc<Cell<u64>>,
    arbiter: Rc<Arbiter>,
    module: Rc<RefCell<Module>>,
    cancel_on: Rc<Cell<Option<u16>>>,
}

// Match firmware's fingerprint::Board and presence::fingerprint_processing;
// transport events run between polls, without emulating physical IRQ timing.
impl Runtime for BoardClock {
    fn now_ms(&self) -> u64 {
        self.now.get()
    }
    fn cancelled(&self) -> bool {
        self.arbiter.cancel_requested()
    }
    fn wait(&mut self) {
        self.now.set(self.now.get() + 1);
        if self
            .cancel_on
            .get()
            .is_some_and(|command| self.module.borrow().calls(command) > 0)
        {
            self.cancel_on.set(None);
            self.arbiter.request_cancel();
        }
    }
    fn processing(&mut self) {
        self.arbiter.set_cancellable(true);
        self.arbiter.set_up_pending(false);
    }
    fn waiting(&mut self, waiting: bool) {
        self.arbiter.set_up_pending(waiting);
    }
    fn end(&mut self) {
        self.arbiter.set_cancellable(false);
        self.arbiter.set_up_pending(false);
    }
}

fn driver() -> (Driver<Wire, BoardClock>, BoardClock) {
    let clock = BoardClock::default();
    let driver = Driver::new(Wire(clock.module.clone()), clock.clone());
    clock.arbiter.set_wait_scope(SCOPE_FIDO);
    (driver, clock)
}

#[test]
fn cancelled_match_keeps_its_cancel_until_dispatch_end_then_new_work_succeeds() {
    let (mut driver, clock) = driver();
    clock.module.borrow_mut().match_slot = 7;
    clock.module.borrow_mut().slots[7] = true;
    driver.prepare(30_000).unwrap();
    let attempt = driver.start(1).unwrap();
    for _ in 0..1_000 {
        if clock.module.borrow().calls(0x0121) > 0 {
            break;
        }
        assert!(driver.poll().is_pending());
        driver.wait();
    }
    assert_eq!(clock.module.borrow().calls(0x0121), 1);
    assert!(clock.arbiter.cancellable_for(SCOPE_FIDO));
    clock.arbiter.request_cancel();
    driver.finish(attempt);
    assert!(!clock.arbiter.cancellable_for(SCOPE_FIDO));
    assert!(!clock.arbiter.pending_for(SCOPE_FIDO));
    for _ in 0..2_000 {
        driver.service_idle();
        if driver.ready() {
            break;
        }
        driver.wait();
    }
    assert!(driver.ready(), "the cancelled operation must be drained");
    assert_eq!(clock.module.borrow().calls(0x0115), 1);
    assert_eq!(clock.module.borrow().calls(0x0122), 1);
    assert_eq!(
        driver.prepare(driver.now_ms() + 30_000),
        Err(rsk_fido::CtapError::KeepAliveCancel),
        "ending or draining the sensor must not clear this request's cancel"
    );

    clock.arbiter.set_wait_scope(SCOPE_NONE);
    driver.release(false);
    clock.arbiter.set_wait_scope(SCOPE_FIDO);
    assert_eq!(
        driver.prepare(driver.now_ms() + 30_000),
        Ok(()),
        "a fresh dispatch must not inherit the drained request's cancel"
    );
    let next = driver.start(1).unwrap();
    assert_ne!(next.id, attempt.id);
    for _ in 0..1_000 {
        if let Poll::Ready(observation) = driver.poll() {
            assert_eq!(observation.attempt, next);
            assert_eq!(observation.result.unwrap().slot, 7);
            driver.finish(next);
            assert_eq!(clock.module.borrow().calls(0x0121), 2);
            return;
        }
        driver.wait();
    }
    panic!("fresh matching did not complete");
}

#[test]
fn cancelled_capture_drains_during_next_dispatch_before_inventory_runs() {
    for (scope, channel) in [(SCOPE_FIDO, 1), (SCOPE_CCID, CHANNEL_NONE)] {
        let (mut driver, clock) = driver();
        clock.module.borrow_mut().slots[99] = true;
        let mut inventory = Inventory::default();
        inventory.set(99, true);
        clock.cancel_on.set(Some(0x0111));
        assert_eq!(
            bio_sensor::execute(
                &mut driver,
                1,
                Command::Capture {
                    slot: 7,
                    generation: 1,
                    first: true,
                    remaining: ENROLL_SAMPLES,
                },
                30_000,
            ),
            Err(rsk_fido::CtapError::KeepAliveCancel)
        );
        assert!(clock.arbiter.cancel_requested());
        assert!(!driver.ready());
        assert_eq!(clock.module.borrow().calls(0x0113), 0);
        clock.arbiter.set_wait_scope(SCOPE_NONE);
        driver.release(false);
        clock.arbiter.set_wait_scope(scope);
        assert_eq!(
            bio_sensor::read_inventory(&mut driver, channel),
            Ok(inventory),
            "a fresh dispatch must drain the cancelled capture and read inventory"
        );
        assert!(driver.ready());
        let module = clock.module.borrow();
        assert_eq!(module.calls(0x0115), 1);
        assert_eq!(module.calls(0x0112), 1);
        assert_eq!(module.calls(0x0133), MODULE_SLOTS);
        assert_eq!(module.calls(0x0113), 0);
    }
}

#[test]
fn a_fresh_cancel_before_prepare_survives_runtime_end_and_blocks_module_io() {
    let (mut driver, clock) = driver();
    driver.prepare(30_000).unwrap();
    clock.arbiter.set_wait_scope(SCOPE_NONE);
    clock.arbiter.request_cancel();
    assert!(
        !clock.arbiter.cancel_requested(),
        "idle cancels own no request"
    );
    clock.arbiter.set_wait_scope(SCOPE_FIDO);
    clock.arbiter.request_cancel();
    let before = clock.module.borrow().commands.len();
    assert_eq!(
        driver.prepare(driver.now_ms() + 30_000),
        Err(rsk_fido::CtapError::KeepAliveCancel)
    );
    assert!(clock.arbiter.cancel_requested());
    assert!(!clock.arbiter.cancellable_for(SCOPE_FIDO));
    assert_eq!(clock.module.borrow().commands.len(), before);
}
