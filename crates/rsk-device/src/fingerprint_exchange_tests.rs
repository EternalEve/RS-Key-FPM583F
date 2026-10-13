// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;
use crate::fingerprint_driver_fixture::{Module, Wire};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn zero_progress_and_wire_busy_have_bounded_distinct_transmission_facts() {
    for stuck in [0, 1, 2] {
        let module = Rc::new(RefCell::new(Module::default()));
        module.borrow_mut().rx.clear();
        if stuck == 0 {
            module.borrow_mut().write_bytes = 0;
        }
        if stuck == 1 {
            module.borrow_mut().busy_wire = true;
        }
        if stuck == 2 {
            module.borrow_mut().read_bytes = 0;
        }
        let mut wire = Wire(module);
        let mut exchange = Exchange::new(Request::Count, 0).unwrap();
        let mut failure = None;
        for now in 0..3_100 {
            if let Poll::Ready(result) = exchange.poll(&mut wire, now) {
                failure = result.err();
                break;
            }
        }
        assert!(failure.is_some());
        assert_eq!(exchange.progress.sent == 0, stuck == 0);
        assert_eq!(exchange.progress.wire_complete, stuck == 2);
        assert!(!exchange.progress.response_complete);
    }
}

#[test]
fn bytes_are_serviced_in_bounded_steps_and_the_response_waits_for_the_wire() {
    let module = Rc::new(RefCell::new(Module::default()));
    module.borrow_mut().rx.clear();
    module.borrow_mut().write_bytes = 16;
    module.borrow_mut().busy_wire = true;
    let mut wire = Wire(module.clone());
    let mut exchange = Exchange::new(Request::Count, 0).unwrap();
    for now in 0..20 {
        assert!(exchange.poll(&mut wire, now).is_pending());
    }
    assert_eq!(exchange.progress.sent, 18);
    assert!(!exchange.progress.wire_complete);
    let waiting = module.borrow().rx.len();
    assert!(waiting > 0);
    module.borrow_mut().busy_wire = false;
    for now in 20..100 {
        if let Poll::Ready(result) = exchange.poll(&mut wire, now) {
            assert!(result.is_ok());
            assert!(exchange.progress.response_complete);
            return;
        }
    }
    panic!("full response was not accepted");
}
