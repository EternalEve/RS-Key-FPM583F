// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;

#[derive(Default)]
struct Bytes {
    written: std::vec::Vec<u8>,
    read: usize,
    pending: bool,
    wire_busy: bool,
    fault: bool,
}

impl Stream for Bytes {
    fn read(&mut self, out: &mut [u8]) -> Poll<Result<usize, Fault>> {
        if self.fault {
            return Poll::Ready(Err(Fault::Communication));
        }
        if self.pending {
            return Poll::Pending;
        }
        let n = out.len().min(3);
        for byte in &mut out[..n] {
            *byte = self.read as u8;
            self.read += 1;
        }
        Poll::Ready(Ok(n))
    }
    fn write(&mut self, bytes: &[u8]) -> Poll<Result<usize, Fault>> {
        if self.pending {
            return Poll::Pending;
        }
        assert!(bytes.len() <= STEP_BYTES);
        let n = bytes.len().min(2);
        self.written.extend_from_slice(&bytes[..n]);
        Poll::Ready(Ok(n))
    }
    fn flush(&mut self) -> Poll<Result<(), Fault>> {
        if self.pending {
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }
    fn busy(&self) -> bool {
        self.wire_busy
    }
}

#[test]
fn partial_io_progresses_and_waits_for_the_wire() {
    let mut bytes = Bytes {
        wire_busy: true,
        ..Default::default()
    };
    let mut response = [0; 20];
    let command = [0xA5; 19];
    let mut transfer = Transfer::new(&command, &mut response, 0);
    for now in 0..40 {
        assert!(transfer.poll_once(&mut bytes, now, false).is_pending());
    }
    assert_eq!(bytes.written, command);
    assert_eq!(
        bytes.read, 0,
        "enqueue and flush cannot authorize reading before wire idle"
    );
    bytes.wire_busy = false;
    let mut result = Poll::Pending;
    for now in 40..60 {
        result = transfer.poll_once(&mut bytes, now, false);
    }
    assert_eq!(result, Poll::Ready(Ok(20)));
    assert_eq!(response, core::array::from_fn(|n| n as u8));
}

#[test]
fn pressure_flush_wire_and_receive_waits_have_deadlines() {
    for phase in [
        Phase::Sending,
        Phase::Flushing,
        Phase::Wire,
        Phase::Receiving,
    ] {
        let mut bytes = Bytes {
            pending: true,
            wire_busy: true,
            ..Default::default()
        };
        let mut response = [0; 2];
        let mut transfer = Transfer::new(&[0xA5], &mut response, 0);
        transfer.phase = phase;
        assert!(
            transfer
                .poll_once(&mut bytes, TX_TIMEOUT_MS - 1, false)
                .is_pending()
        );
        assert_eq!(
            transfer.poll_once(&mut bytes, TX_TIMEOUT_MS, false),
            Poll::Ready(Err(Fault::Timeout))
        );
        bytes.pending = false;
        assert_eq!(
            transfer.poll_once(&mut bytes, TX_TIMEOUT_MS + 1, false),
            Poll::Ready(Err(Fault::Timeout))
        );
    }
}

#[test]
fn cancellation_and_uart_error_never_complete_a_transfer() {
    let mut bytes = Bytes::default();
    let mut response = [0; 2];
    let mut transfer = Transfer::new(&[0xA5], &mut response, 0);
    assert_eq!(
        transfer.poll_once(&mut bytes, 1, true),
        Poll::Ready(Err(Fault::Cancelled))
    );
    assert_eq!(
        transfer.poll_once(&mut bytes, 2, false),
        Poll::Ready(Err(Fault::Cancelled))
    );
    let mut transfer = Transfer::new(&[], &mut response, 0);
    transfer.phase = Phase::Receiving;
    bytes.fault = true;
    assert_eq!(
        transfer.poll_once(&mut bytes, 1, false),
        Poll::Ready(Err(Fault::Communication))
    );
}

#[test]
fn receive_deadline_begins_after_wire_completion_and_never_slides() {
    let mut bytes = Bytes {
        wire_busy: true,
        ..Default::default()
    };
    let mut response = [0; 8];
    let mut transfer = Transfer::new(&[], &mut response, 0);
    assert!(transfer.poll_once(&mut bytes, 0, false).is_pending());
    assert!(transfer.poll_once(&mut bytes, 1, false).is_pending());
    bytes.wire_busy = false;
    assert!(
        transfer
            .poll_once(&mut bytes, TX_TIMEOUT_MS - 1, false)
            .is_pending()
    );
    bytes.pending = true;
    let deadline = TX_TIMEOUT_MS - 1 + RX_TIMEOUT_MS;
    assert!(
        transfer
            .poll_once(&mut bytes, deadline - 1, false)
            .is_pending()
    );
    assert_eq!(
        transfer.poll_once(&mut bytes, deadline, false),
        Poll::Ready(Err(Fault::Timeout))
    );
    bytes.pending = false;
    assert_eq!(
        transfer.poll_once(&mut bytes, deadline + 1, false),
        Poll::Ready(Err(Fault::Timeout))
    );
}

#[test]
fn a_clock_rollback_and_cancel_during_wire_wait_are_terminal() {
    let mut bytes = Bytes {
        wire_busy: true,
        ..Default::default()
    };
    for rollback in [false, true] {
        let mut response = [0; 1];
        let mut transfer = Transfer::new(&[], &mut response, 10);
        assert!(transfer.poll_once(&mut bytes, 10, false).is_pending());
        assert!(transfer.poll_once(&mut bytes, 11, false).is_pending());
        let fault = if rollback {
            Fault::Timeout
        } else {
            Fault::Cancelled
        };
        assert_eq!(
            transfer.poll_once(&mut bytes, if rollback { 9 } else { 12 }, !rollback),
            Poll::Ready(Err(fault))
        );
        bytes.wire_busy = false;
        assert_eq!(
            transfer.poll_once(&mut bytes, 13, false),
            Poll::Ready(Err(fault))
        );
    }
}
