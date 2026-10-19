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
    (
        Driver::new(Wire(module.clone()), clock.clone()),
        module,
        clock,
    )
}

fn settle<U: Stream>(driver: &mut Driver<U, Clock>) {
    for _ in 0..4_000 {
        driver.service_idle();
        if matches!(driver.power_phase(), Phase::Off | Phase::Unavailable) {
            return;
        }
        driver.wait();
    }
    panic!("shutdown did not finish within its budget");
}

struct StartingWire {
    wire: Wire,
    clock: Clock,
    ready_at: u64,
    accesses: Rc<RefCell<Vec<u64>>>,
    uart_enabled: bool,
    fail_enable: bool,
    enable_delay: u64,
}

impl Stream for StartingWire {
    fn read(&mut self, bytes: &mut [u8]) -> Poll<Result<usize, crate::fingerprint_io::Fault>> {
        let now = self.clock.0.get();
        self.accesses.borrow_mut().push(now);
        if now < self.ready_at || !self.uart_enabled {
            return Poll::Ready(Err(crate::fingerprint_io::Fault::Communication));
        }
        self.wire.read(bytes)
    }
    fn write(&mut self, bytes: &[u8]) -> Poll<Result<usize, crate::fingerprint_io::Fault>> {
        self.wire.write(bytes)
    }
    fn flush(&mut self) -> Poll<Result<(), crate::fingerprint_io::Fault>> {
        self.wire.flush()
    }
    fn busy(&self) -> bool {
        self.wire.busy()
    }
    fn power_controlled(&self) -> bool {
        true
    }
    fn power_on(&mut self) -> Result<(), crate::fingerprint_io::Fault> {
        self.ready_at = self.clock.0.get() + BOOT_DELAY_MS;
        self.uart_enabled = false;
        self.wire.power_on()
    }
    fn enable_uart(&mut self) -> Result<(), crate::fingerprint_io::Fault> {
        assert!(self.clock.0.get() >= self.ready_at);
        if self.fail_enable {
            return Err(crate::fingerprint_io::Fault::Communication);
        }
        self.clock.0.set(self.clock.0.get() + self.enable_delay);
        self.uart_enabled = true;
        Ok(())
    }
    fn power_off(&mut self) -> Result<(), crate::fingerprint_io::Fault> {
        self.wire.power_off()
    }
}

#[test]
fn supply_startup_finishes_before_the_first_uart_access() {
    let (base, module, clock) = driver();
    let accesses = Rc::new(RefCell::new(Vec::new()));
    let wire = StartingWire {
        wire: base.stream,
        clock: clock.clone(),
        ready_at: 0,
        accesses: accesses.clone(),
        uart_enabled: false,
        fail_enable: false,
        enable_delay: 0,
    };
    let mut driver = Driver::new(wire, clock);
    for _ in 0..2 {
        accesses.borrow_mut().clear();
        let started = driver.now_ms();
        assert_eq!(
            driver.prepare(started + 30_000),
            Ok(()),
            "a valid module must boot without reading its unsettled UART"
        );
        assert!(!accesses.borrow().is_empty());
        assert!(
            accesses
                .borrow()
                .iter()
                .all(|t| *t >= started + BOOT_DELAY_MS)
        );
        assert!(module.borrow().calls(0x0203) > 0);
        driver.release(false);
        settle(&mut driver);
        assert_eq!(driver.power_phase(), Phase::Off);
    }
}

#[test]
fn failed_uart_initialization_never_sends_or_automatically_restarts() {
    let (base, module, clock) = driver();
    let wire = StartingWire {
        wire: base.stream,
        clock: clock.clone(),
        ready_at: 0,
        accesses: Rc::new(RefCell::new(Vec::new())),
        uart_enabled: false,
        fail_enable: true,
        enable_delay: 0,
    };
    let mut driver = Driver::new(wire, clock);
    for _ in 0..2 {
        assert_eq!(driver.prepare(30_000), Err(rsk_fido::CtapError::Other));
        assert_eq!(driver.power_phase(), Phase::Unavailable);
        assert!(module.borrow().commands.is_empty());
        assert_eq!(module.borrow().cycles, 1);
        assert_eq!(module.borrow().cuts, 0);
        assert!(driver.stream.accesses.borrow().is_empty());
    }
}

#[test]
fn a_short_request_deadline_does_not_enable_uart_during_the_supply_delay() {
    let (base, module, clock) = driver();
    let wire = StartingWire {
        wire: base.stream,
        clock: clock.clone(),
        ready_at: 0,
        accesses: Rc::new(RefCell::new(Vec::new())),
        uart_enabled: false,
        fail_enable: false,
        enable_delay: 0,
    };
    let mut driver = Driver::new(wire, clock);
    assert_eq!(
        driver.prepare(BOOT_DELAY_MS / 2),
        Err(rsk_fido::CtapError::UserActionTimeout)
    );
    assert!(!driver.stream.uart_enabled);
    assert!(driver.stream.accesses.borrow().is_empty());
    assert!(module.borrow().commands.is_empty());
    assert_eq!(module.borrow().cuts, 0);
}

#[test]
fn uart_initialization_reaching_the_deadline_does_not_send_a_boot_query() {
    let (base, module, clock) = driver();
    let wire = StartingWire {
        wire: base.stream,
        clock: clock.clone(),
        ready_at: 0,
        accesses: Rc::new(RefCell::new(Vec::new())),
        uart_enabled: false,
        fail_enable: false,
        enable_delay: BOOT_TIMEOUT_MS - BOOT_DELAY_MS,
    };
    let mut driver = Driver::new(wire, clock);
    assert_eq!(driver.prepare(30_000), Err(rsk_fido::CtapError::Other));
    assert_eq!(driver.power_phase(), Phase::Unavailable);
    assert!(driver.stream.accesses.borrow().is_empty());
    assert!(module.borrow().commands.is_empty());
    assert_eq!(module.borrow().cuts, 0);
}

struct LateWire {
    wire: Wire,
    clock: Clock,
    late: Option<(u16, u64)>,
}

impl Stream for LateWire {
    fn read(&mut self, bytes: &mut [u8]) -> Poll<Result<usize, crate::fingerprint_io::Fault>> {
        let result = self.wire.read(bytes);
        if let Some((command, deadline)) = self.late
            && matches!(result, Poll::Ready(Ok(_)))
            && self.wire.0.borrow().rx.is_empty()
            && self
                .wire
                .0
                .borrow()
                .commands
                .last()
                .is_some_and(|(c, _)| *c == command)
        {
            self.clock.0.set(deadline);
            self.late = None;
        }
        result
    }
    fn write(&mut self, bytes: &[u8]) -> Poll<Result<usize, crate::fingerprint_io::Fault>> {
        self.wire.write(bytes)
    }
    fn flush(&mut self) -> Poll<Result<(), crate::fingerprint_io::Fault>> {
        self.wire.flush()
    }
    fn busy(&self) -> bool {
        self.wire.busy()
    }
    fn power_controlled(&self) -> bool {
        self.wire.power_controlled()
    }
    fn power_on(&mut self) -> Result<(), crate::fingerprint_io::Fault> {
        self.wire.power_on()
    }
    fn power_off(&mut self) -> Result<(), crate::fingerprint_io::Fault> {
        self.wire.power_off()
    }
}

#[test]
fn a_reply_finishing_on_the_startup_or_shutdown_deadline_is_not_accepted() {
    for sleep in [false, true] {
        let (driver, module, clock) = driver();
        let wire = LateWire {
            wire: driver.stream,
            clock: clock.clone(),
            late: (!sleep).then_some((0x0203, BOOT_TIMEOUT_MS)),
        };
        let mut driver = Driver::new(wire, clock);
        if sleep {
            driver.prepare(10_000).unwrap();
            driver.stream.late = Some((
                0x020c,
                driver.now_ms() + crate::fingerprint_power::STOP_TIMEOUT_MS,
            ));
            driver.release(false);
            settle(&mut driver);
        } else {
            assert_eq!(
                driver.prepare(10_000),
                Err(rsk_fido::CtapError::Other),
                "a boot reply reaching the deadline must be refused"
            );
        }
        assert_eq!(driver.power_phase(), Phase::Unavailable);
        assert_eq!(
            module.borrow().cuts,
            0,
            "a late sleep reply must not remove power"
        );
        assert_eq!(module.borrow().cycles, 1);
    }
}

#[test]
fn idle_capabilities_and_repeated_cycles_do_not_reset_operation_ids_or_templates() {
    let (mut driver, module, _) = driver();
    module.borrow_mut().slots[7] = true;
    for _ in 0..20 {
        driver.service_idle();
    }
    assert_eq!(driver.power_phase(), Phase::Off);
    assert_eq!(driver.capabilities().unwrap().samples, ENROLL_SAMPLES);
    assert!(module.borrow().commands.is_empty());
    let mut previous = 0;
    for _ in 0..3 {
        driver.prepare(driver.now_ms() + 30_000).unwrap();
        let attempt = driver.start(1).unwrap();
        assert!(attempt.id > previous);
        previous = attempt.id;
        loop {
            if let Poll::Ready(observation) = driver.poll() {
                assert_eq!(observation.attempt, attempt);
                assert!(observation.result.is_ok());
                break;
            }
            driver.wait();
        }
        driver.finish(attempt);
        driver.release(false);
        settle(&mut driver);
        assert_eq!(driver.power_phase(), Phase::Off);
        assert!(module.borrow().slots[7]);
        assert!(module.borrow().rx.is_empty());
    }
    assert_eq!((module.borrow().cycles, module.borrow().cuts), (3, 3));
    assert_eq!(module.borrow().calls(0x020c), 3);
}

#[test]
fn request_and_six_sample_session_holds_prevent_premature_sleep() {
    let (mut driver, module, _) = driver();
    driver.prepare(30_000).unwrap();
    for remaining in (1..=6).rev() {
        let result = bio_sensor::execute(
            &mut driver,
            1,
            Command::Capture {
                slot: 7,
                generation: 1,
                first: remaining == 6,
                remaining,
            },
            30_000,
        )
        .unwrap();
        assert!(
            matches!(result, bio_sensor::Result::Sample { saved, .. } if saved == (remaining == 1))
        );
        // The same request still needs its post-write inventory and durable commit.
        for _ in 0..20 {
            driver.service_idle();
        }
        assert_eq!(module.borrow().cuts, 0);
        driver.release(remaining > 1);
        if remaining > 1 {
            for _ in 0..100 {
                driver.service_idle();
                driver.wait();
            }
            assert_eq!(
                module.borrow().cuts,
                0,
                "a capture gap must preserve temporary samples"
            );
            driver.prepare(driver.now_ms() + 30_000).unwrap();
        }
    }
    assert_eq!(module.borrow().good_samples, 6);
    settle(&mut driver);
    assert_eq!(module.borrow().cuts, 1);
    assert_eq!(module.borrow().cycles, 1);
    assert!(module.borrow().slots[7]);
}

#[test]
fn lost_sleep_or_unknown_write_never_cuts_power_or_restarts() {
    for lost in [0x020c, 0x0132, 0x0114] {
        let (mut driver, module, _) = driver();
        driver.prepare(30_000).unwrap();
        module.borrow_mut().drop_command = Some(lost);
        match lost {
            0x0132 => {
                module.borrow_mut().slots[7] = true;
                assert!(
                    bio_sensor::execute(&mut driver, 1, Command::Delete { slot: 7 }, 30_000)
                        .is_err()
                );
                assert_eq!(module.borrow().calls(0x0131), 1);
            }
            0x0114 => {
                for remaining in (2..=6).rev() {
                    bio_sensor::execute(
                        &mut driver,
                        1,
                        Command::Capture {
                            slot: 7,
                            generation: 1,
                            first: remaining == 6,
                            remaining,
                        },
                        30_000,
                    )
                    .unwrap();
                }
                assert!(
                    bio_sensor::execute(
                        &mut driver,
                        1,
                        Command::Capture {
                            slot: 7,
                            generation: 1,
                            first: false,
                            remaining: 1,
                        },
                        30_000
                    )
                    .is_err()
                );
                assert_eq!(module.borrow().calls(0x0113), 1);
            }
            _ => {}
        }
        driver.release(false);
        settle(&mut driver);
        assert_eq!(driver.power_phase(), Phase::Unavailable);
        assert_eq!(
            module.borrow().cuts,
            0,
            "an uncertain terminal result forbids a power cut"
        );
        assert_eq!(
            driver.prepare(driver.now_ms() + 30_000),
            Err(rsk_fido::CtapError::Other)
        );
        assert_eq!(module.borrow().cycles, 1);
        assert_eq!(module.borrow().calls(0x020c), usize::from(lost == 0x020c));
    }
}

#[test]
fn startup_deadline_cancellation_and_no_lift_are_bounded_without_matching() {
    let (mut driver, module, clock) = driver();
    assert_eq!(
        driver.prepare(100),
        Err(rsk_fido::CtapError::UserActionTimeout)
    );
    assert_eq!(clock.0.get(), 100);
    assert_eq!(module.borrow().calls(0x0121), 0);
    driver.release(false);
    settle(&mut driver);
    assert_eq!(module.borrow().cuts, 0);

    let (mut driver, module, clock) = self::driver();
    clock.1.set(true);
    assert_eq!(
        driver.prepare(1_000),
        Err(rsk_fido::CtapError::KeepAliveCancel)
    );
    assert_eq!(module.borrow().cycles, 0);
    clock.1.set(false);
    driver.prepare(30_000).unwrap();
    module.borrow_mut().finger_present = true;
    let attempt = driver.start(1).unwrap();
    for _ in 0..500 {
        let _ = driver.poll();
        driver.wait();
    }
    assert_eq!(
        module.borrow().calls(0x0121),
        0,
        "cold start must not accept a finger already present"
    );
    driver.finish(attempt);
    driver.release(false);
    settle(&mut driver);
    assert_eq!(module.borrow().cuts, 1);
}
