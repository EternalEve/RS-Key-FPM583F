// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Short driver borrows leave presence waits free to service ordinary sleep.

use core::cell::RefCell;
use core::task::Poll;

use rsk_fido::CtapError;
use rsk_fido::bio_sensor::{Capabilities, Command};
use rsk_fido::fingerprint::{Attempt, Failure, Matching, Observation, Sensor};

use crate::fingerprint_driver::{Driver, Runtime};
use crate::fingerprint_io::Stream;

pub struct Service<'a, U, C> {
    driver: &'a RefCell<Driver<U, C>>,
}

impl<'a, U: Stream, C: Runtime> Service<'a, U, C> {
    pub fn new(driver: &'a RefCell<Driver<U, C>>) -> Self {
        Self { driver }
    }
}

impl<U: Stream, C: Runtime> Sensor for Service<'_, U, C> {
    fn now_ms(&self) -> u64 {
        self.driver.borrow().now_ms()
    }
    fn ready(&self) -> bool {
        self.driver.borrow().ready()
    }
    fn start(&mut self, channel: u32) -> Result<Attempt, Failure> {
        self.driver.borrow_mut().start(channel)
    }
    fn poll(&mut self) -> Poll<Observation> {
        self.driver.borrow_mut().poll()
    }
    fn cancelled(&self) -> bool {
        self.driver.borrow().cancelled()
    }
    fn matching(&self) -> Matching {
        self.driver.borrow().matching()
    }
    fn wait(&mut self) {
        self.driver.borrow_mut().wait();
    }
    fn finish(&mut self, attempt: Attempt) {
        self.driver.borrow_mut().finish(attempt);
    }
    fn service_idle(&mut self) {
        self.driver.borrow_mut().service_idle();
    }
    fn prepare(&mut self, deadline_ms: u64) -> Result<(), CtapError> {
        self.driver.borrow_mut().prepare(deadline_ms)
    }
    fn release(&mut self, enrollment: bool) {
        self.driver.borrow_mut().release(enrollment);
    }
    fn deadline_ms(&self, deadline_ms: u64) -> u64 {
        self.driver.borrow().deadline_ms(deadline_ms)
    }
    fn capabilities(&self) -> Option<Capabilities> {
        self.driver.borrow().capabilities()
    }
    fn start_management(&mut self, channel: u32, command: Command) -> Result<Attempt, Failure> {
        self.driver.borrow_mut().start_management(channel, command)
    }
    fn poll_management(&mut self) -> Poll<rsk_fido::bio_sensor::Observation> {
        self.driver.borrow_mut().poll_management()
    }
}
