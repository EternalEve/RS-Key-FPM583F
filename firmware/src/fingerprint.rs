// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! IRQ-buffered UART ownership; the confirmed module codec comes in phase 5.

use core::future::Future;
use core::task::{Context, Poll, Waker};

use embassy_rp::peripherals::{PIN_0, PIN_1, UART0};
use embassy_rp::uart::{BufferedInterruptHandler, BufferedUart, Config};
use embassy_rp::{Peri, bind_interrupts};
use embedded_io_async::{Read, Write};
use rsk_device::fingerprint_io::{Fault, SERVICE_INTERVAL_MS, STEP_BYTES, Stream};
use rsk_fido::fingerprint::{Attempt, Failure, Matching, Observation, Sensor};
use static_cell::StaticCell;

const BAUD: u32 = 57_600;
const TX_BYTES: usize = 128;
const RX_BYTES: usize = 256;

bind_interrupts!(struct Irqs {
    UART0_IRQ => BufferedInterruptHandler<UART0>;
});

fn poll_once<F: Future>(future: F) -> Poll<F::Output> {
    let mut future = core::pin::pin!(future);
    future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
}

struct Uart(BufferedUart);

impl Stream for Uart {
    fn read(&mut self, bytes: &mut [u8]) -> Poll<Result<usize, Fault>> {
        poll_once(Read::read(&mut self.0, bytes)).map(|r| r.map_err(|_| Fault::Communication))
    }
    fn write(&mut self, bytes: &[u8]) -> Poll<Result<usize, Fault>> {
        poll_once(Write::write(&mut self.0, bytes)).map(|r| r.map_err(|_| Fault::Communication))
    }
    fn flush(&mut self) -> Poll<Result<(), Fault>> {
        poll_once(Write::flush(&mut self.0)).map(|r| r.map_err(|_| Fault::Communication))
    }
    fn busy(&self) -> bool {
        self.0.busy()
    }
}

pub struct Bridge {
    uart: Uart,
    fault: Option<Fault>,
}

impl Bridge {
    pub fn new(
        uart: Peri<'static, UART0>,
        tx: Peri<'static, PIN_0>,
        rx: Peri<'static, PIN_1>,
    ) -> Self {
        static TX: StaticCell<[u8; TX_BYTES]> = StaticCell::new();
        static RX: StaticCell<[u8; RX_BYTES]> = StaticCell::new();
        let mut config = Config::default();
        config.baudrate = BAUD;
        Self {
            uart: Uart(BufferedUart::new(
                uart,
                tx,
                rx,
                Irqs,
                TX.init([0; TX_BYTES]),
                RX.init([0; RX_BYTES]),
                config,
            )),
            fault: None,
        }
    }
}

impl Sensor for Bridge {
    fn now_ms(&self) -> u64 {
        crate::usb_attach::elapsed_ms()
    }
    fn ready(&self) -> bool {
        false
    }
    fn start(&mut self, _channel: u32) -> Result<Attempt, Failure> {
        Err(Failure::Unavailable)
    }
    fn poll(&mut self) -> Poll<Observation> {
        Poll::Pending
    }
    fn cancelled(&self) -> bool {
        crate::presence::cancel_requested()
    }
    fn matching(&self) -> Matching {
        Matching::NotStarted
    }
    fn wait(&mut self) {
        embassy_time::block_for(embassy_time::Duration::from_millis(SERVICE_INTERVAL_MS));
    }
    fn finish(&mut self, _attempt: Attempt) {}
    fn service_idle(&mut self) {
        if self.fault.is_none() {
            let mut bytes = [0; STEP_BYTES];
            if let Poll::Ready(Err(fault)) = self.uart.read(&mut bytes) {
                self.fault = Some(fault);
            }
        }
    }
}
