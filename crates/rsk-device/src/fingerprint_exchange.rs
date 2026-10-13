// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! One outstanding UART request; a response cannot reopen a timed-out exchange.

use core::task::Poll;

use crate::fingerprint_io::{Fault, RX_TIMEOUT_MS, STEP_BYTES, Stream, TX_TIMEOUT_MS};
use crate::fingerprint_protocol::{Completion, Decoder, Error, Frame, Request};

const MAX_REQUEST: usize = 21;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Transport(Fault),
    Protocol(Error),
    ExtraResponse,
    Module(Completion),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub sent: usize,
    pub wire_complete: bool,
    pub response_complete: bool,
}

enum Phase {
    Sending,
    Flushing,
    Wire,
    Receiving,
    Boundary(Frame),
    Complete(Frame),
    Failed(Failure),
}

pub(crate) struct Exchange {
    bytes: [u8; MAX_REQUEST],
    len: usize,
    decoder: Decoder,
    phase: Phase,
    deadline_ms: u64,
    last_ms: u64,
    pub progress: Progress,
}

impl Exchange {
    pub fn new(request: Request, now_ms: u64) -> Result<Self, Error> {
        let mut bytes = [0; MAX_REQUEST];
        let len = request.encode([0; 4], &mut bytes)?;
        Ok(Self {
            bytes,
            len,
            decoder: Decoder::new(),
            phase: Phase::Sending,
            deadline_ms: now_ms.saturating_add(TX_TIMEOUT_MS),
            last_ms: now_ms,
            progress: Progress {
                sent: 0,
                wire_complete: false,
                response_complete: false,
            },
        })
    }

    pub fn poll(&mut self, stream: &mut impl Stream, now_ms: u64) -> Poll<Result<Frame, Failure>> {
        match self.phase {
            Phase::Complete(frame) => return Poll::Ready(Ok(frame)),
            Phase::Failed(failure) => return Poll::Ready(Err(failure)),
            _ => {}
        }
        let result = if now_ms < self.last_ms || now_ms >= self.deadline_ms {
            Poll::Ready(Err(Failure::Transport(Fault::Timeout)))
        } else {
            self.step(stream, now_ms)
        };
        self.last_ms = now_ms;
        if let Poll::Ready(Err(failure)) = result {
            self.phase = Phase::Failed(failure);
        }
        result
    }

    fn step(&mut self, stream: &mut impl Stream, now_ms: u64) -> Poll<Result<Frame, Failure>> {
        match self.phase {
            Phase::Sending if self.progress.sent == self.len => {
                self.phase = Phase::Flushing;
            }
            Phase::Sending => {
                let end = (self.progress.sent + STEP_BYTES).min(self.len);
                match stream.write(&self.bytes[self.progress.sent..end]) {
                    Poll::Ready(Ok(n)) if n > 0 && n <= end - self.progress.sent => {
                        self.progress.sent += n
                    }
                    Poll::Ready(Ok(_)) => {
                        return Poll::Ready(Err(Failure::Transport(Fault::Communication)));
                    }
                    Poll::Ready(Err(fault)) => return Poll::Ready(Err(Failure::Transport(fault))),
                    Poll::Pending => {}
                }
            }
            Phase::Flushing => match stream.flush() {
                Poll::Ready(Ok(())) => self.phase = Phase::Wire,
                Poll::Ready(Err(fault)) => return Poll::Ready(Err(Failure::Transport(fault))),
                Poll::Pending => {}
            },
            Phase::Wire if !stream.busy() => {
                self.progress.wire_complete = true;
                self.phase = Phase::Receiving;
                self.deadline_ms = now_ms.saturating_add(RX_TIMEOUT_MS);
            }
            Phase::Receiving => return self.receive(stream),
            Phase::Boundary(frame) => {
                let mut extra = [0; STEP_BYTES];
                match stream.read(&mut extra) {
                    Poll::Pending if !stream.busy() => {
                        self.phase = Phase::Complete(frame);
                        return Poll::Ready(Ok(frame));
                    }
                    Poll::Pending => {}
                    Poll::Ready(Err(fault)) => return Poll::Ready(Err(Failure::Transport(fault))),
                    Poll::Ready(Ok(_)) => return Poll::Ready(Err(Failure::ExtraResponse)),
                }
            }
            _ => {}
        }
        Poll::Pending
    }

    fn receive(&mut self, stream: &mut impl Stream) -> Poll<Result<Frame, Failure>> {
        let mut bytes = [0; STEP_BYTES];
        let n = match stream.read(&mut bytes) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(Err(fault)) => return Poll::Ready(Err(Failure::Transport(fault))),
            Poll::Ready(Ok(n)) if n > 0 && n <= bytes.len() => n,
            Poll::Ready(Ok(_)) => {
                return Poll::Ready(Err(Failure::Transport(Fault::Communication)));
            }
        };
        for (i, byte) in bytes[..n].iter().enumerate() {
            match self.decoder.feed(*byte) {
                Some(Err(error)) => return Poll::Ready(Err(Failure::Protocol(error))),
                Some(Ok(frame)) if i + 1 == n => {
                    self.progress.response_complete = true;
                    self.phase = Phase::Boundary(frame);
                }
                Some(Ok(_)) => return Poll::Ready(Err(Failure::ExtraResponse)),
                None => {}
            }
        }
        Poll::Pending
    }
}

#[cfg(test)]
#[path = "fingerprint_exchange_tests.rs"]
mod tests;
