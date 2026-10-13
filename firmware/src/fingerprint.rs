// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Physical UART and clock for the host-tested FPM583F service.

use core::future::Future;
use core::task::{Context, Poll, Waker};

use embassy_rp::peripherals::{PIN_0, PIN_1, UART0};
use embassy_rp::uart::{BufferedInterruptHandler, BufferedUart, Config};
use embassy_rp::{Peri, bind_interrupts};
use embedded_io_async::{Read, Write};
use rsk_device::fingerprint_driver::{Driver, Runtime};
use rsk_device::fingerprint_io::{Fault, SERVICE_INTERVAL_MS, Stream};
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

pub struct Uart(BufferedUart);

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

pub struct Board;

impl Runtime for Board {
    fn now_ms(&self) -> u64 {
        crate::usb_attach::elapsed_ms()
    }
    fn cancelled(&self) -> bool {
        crate::presence::cancel_requested()
    }
    fn wait(&mut self) {
        embassy_time::block_for(embassy_time::Duration::from_millis(SERVICE_INTERVAL_MS));
    }
    fn processing(&mut self) {
        crate::presence::fingerprint_processing(true);
    }
    fn waiting(&mut self, waiting: bool) {
        crate::presence::set_up_pending(waiting);
    }
    fn end(&mut self) {
        crate::presence::fingerprint_processing(false);
    }
}

pub type Bridge = Driver<Uart, Board>;

pub fn bridge(
    uart: Peri<'static, UART0>,
    tx: Peri<'static, PIN_0>,
    rx: Peri<'static, PIN_1>,
) -> Bridge {
    static TX: StaticCell<[u8; TX_BYTES]> = StaticCell::new();
    static RX: StaticCell<[u8; RX_BYTES]> = StaticCell::new();
    let mut config = Config::default();
    config.baudrate = BAUD;
    Driver::new(
        Uart(BufferedUart::new(
            uart,
            tx,
            rx,
            Irqs,
            TX.init([0; TX_BYTES]),
            RX.init([0; RX_BYTES]),
            config,
        )),
        Board,
    )
}
