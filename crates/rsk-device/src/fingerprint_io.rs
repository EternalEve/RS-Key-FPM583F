// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Bounded byte transfers; command framing belongs to the module driver.

use core::task::Poll;

pub const STEP_BYTES: usize = 16;
pub const SERVICE_INTERVAL_MS: u64 = 1;
pub const TX_TIMEOUT_MS: u64 = 1_000;
pub const RX_TIMEOUT_MS: u64 = 2_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    Communication,
    Timeout,
    Cancelled,
}

pub trait Stream {
    fn read(&mut self, bytes: &mut [u8]) -> Poll<Result<usize, Fault>>;
    fn write(&mut self, bytes: &[u8]) -> Poll<Result<usize, Fault>>;
    fn flush(&mut self) -> Poll<Result<(), Fault>>;
    fn busy(&self) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Sending,
    Flushing,
    Wire,
    Receiving,
    Complete,
    Failed(Fault),
}

pub struct Transfer<'a> {
    command: &'a [u8],
    response: &'a mut [u8],
    sent: usize,
    received: usize,
    phase: Phase,
    deadline_ms: u64,
    last_ms: u64,
}

impl<'a> Transfer<'a> {
    pub fn new(command: &'a [u8], response: &'a mut [u8], now_ms: u64) -> Self {
        Self {
            command,
            response,
            sent: 0,
            received: 0,
            phase: Phase::Sending,
            deadline_ms: now_ms.saturating_add(TX_TIMEOUT_MS),
            last_ms: now_ms,
        }
    }

    pub fn poll_once(
        &mut self,
        stream: &mut impl Stream,
        now_ms: u64,
        cancelled: bool,
    ) -> Poll<Result<usize, Fault>> {
        if self.phase == Phase::Complete {
            return Poll::Ready(Ok(self.received));
        }
        if let Phase::Failed(fault) = self.phase {
            return Poll::Ready(Err(fault));
        }
        let fault = if cancelled {
            Some(Fault::Cancelled)
        } else if now_ms < self.last_ms || now_ms >= self.deadline_ms {
            Some(Fault::Timeout)
        } else {
            None
        };
        self.last_ms = now_ms;
        if let Some(fault) = fault {
            self.phase = Phase::Failed(fault);
            return Poll::Ready(Err(fault));
        }
        let result = match self.phase {
            Phase::Sending if self.sent == self.command.len() => {
                self.phase = Phase::Flushing;
                Poll::Pending
            }
            Phase::Sending => {
                let end = (self.sent + STEP_BYTES).min(self.command.len());
                match stream.write(&self.command[self.sent..end]) {
                    Poll::Ready(Ok(n)) if n > 0 && n <= end - self.sent => {
                        self.sent += n;
                        Poll::Pending
                    }
                    Poll::Ready(Ok(_)) => Poll::Ready(Err(Fault::Communication)),
                    Poll::Ready(Err(fault)) => Poll::Ready(Err(fault)),
                    Poll::Pending => Poll::Pending,
                }
            }
            Phase::Flushing => match stream.flush() {
                Poll::Ready(Ok(())) => {
                    self.phase = Phase::Wire;
                    Poll::Pending
                }
                Poll::Ready(Err(fault)) => Poll::Ready(Err(fault)),
                Poll::Pending => Poll::Pending,
            },
            Phase::Wire if !stream.busy() => {
                self.phase = Phase::Receiving;
                self.deadline_ms = now_ms.saturating_add(RX_TIMEOUT_MS);
                Poll::Pending
            }
            Phase::Receiving if self.received == self.response.len() => {
                self.phase = Phase::Complete;
                Poll::Ready(Ok(self.received))
            }
            Phase::Receiving => {
                let end = (self.received + STEP_BYTES).min(self.response.len());
                match stream.read(&mut self.response[self.received..end]) {
                    Poll::Ready(Ok(n)) if n > 0 && n <= end - self.received => {
                        self.received += n;
                        Poll::Pending
                    }
                    Poll::Ready(Ok(_)) => Poll::Ready(Err(Fault::Communication)),
                    Poll::Ready(Err(fault)) => Poll::Ready(Err(fault)),
                    Poll::Pending => Poll::Pending,
                }
            }
            _ => Poll::Pending,
        };
        if let Poll::Ready(Err(fault)) = result {
            self.phase = Phase::Failed(fault);
        }
        result
    }
}

#[cfg(test)]
#[path = "fingerprint_io_tests.rs"]
mod tests;
