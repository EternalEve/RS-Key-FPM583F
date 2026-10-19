// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Physical UART and clock for the host-tested FPM583F service.

use core::future::Future;
use core::task::{Context, Poll, Waker};

use embassy_rp::gpio::{Flex, Output, Pull};
use embassy_rp::interrupt::InterruptExt;
use embassy_rp::peripherals::{PIN_0, PIN_1, UART0};
use embassy_rp::uart::{BufferedInterruptHandler, BufferedUart, Config};
use embassy_rp::{Peri, bind_interrupts};
use embedded_io_async::{Read, Write};
use rsk_device::fingerprint_driver::{Driver, Runtime};
use rsk_device::fingerprint_io::{Fault, SERVICE_INTERVAL_MS, Stream};
use rsk_vendor::fingerprint::{Stage, UartFault};
use static_cell::StaticCell;

#[path = "fingerprint_status.rs"]
pub(crate) mod status;

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

pub struct Uart {
    active: Option<BufferedUart>,
    uart: Peri<'static, UART0>,
    tx: Peri<'static, PIN_0>,
    rx: Peri<'static, PIN_1>,
    tx_bytes: &'static mut [u8; TX_BYTES],
    rx_bytes: &'static mut [u8; RX_BYTES],
    power: Option<Output<'static>>,
}

impl Uart {
    fn enable(&mut self) -> Result<(), Fault> {
        if self.active.is_some() {
            return Ok(());
        }
        status::stage(Stage::UartStarting);
        rp_pac::RESETS.reset().modify(|w| w.set_uart0(false));
        const RESET_POLLS: usize = 1_024;
        for _ in 0..RESET_POLLS {
            if rp_pac::RESETS.reset_done().read().uart0() {
                let mut config = Config::default();
                config.baudrate = BAUD;
                self.active = Some(BufferedUart::new(
                    self.uart.reborrow(),
                    self.tx.reborrow(),
                    self.rx.reborrow(),
                    Irqs,
                    &mut *self.tx_bytes,
                    &mut *self.rx_bytes,
                    config,
                ));
                status::stage(Stage::UartReady);
                return Ok(());
            }
            cortex_m::asm::nop();
        }
        status::uart_fault(UartFault::Reset);
        Err(Fault::Communication)
    }

    fn disable(&mut self) {
        cortex_m::interrupt::free(|_| {
            embassy_rp::interrupt::UART0_IRQ.disable();
            drop(self.active.take());
            rp_pac::RESETS.reset().modify(|w| w.set_uart0(true));
            embassy_rp::interrupt::UART0_IRQ.unpend();
            let mut tx = Flex::new(self.tx.reborrow());
            let mut rx = Flex::new(self.rx.reborrow());
            tx.set_as_input();
            rx.set_as_input();
            tx.set_pull(Pull::None);
            rx.set_pull(Pull::None);
        });
    }
}

fn uart_error(error: embassy_rp::uart::Error) -> Fault {
    let fault = match error {
        embassy_rp::uart::Error::Overrun => UartFault::Overrun,
        embassy_rp::uart::Error::Break => UartFault::Break,
        embassy_rp::uart::Error::Parity => UartFault::Parity,
        embassy_rp::uart::Error::Framing => UartFault::Framing,
        _ => UartFault::Unknown,
    };
    status::uart_fault(fault);
    Fault::Communication
}

impl Stream for Uart {
    fn read(&mut self, bytes: &mut [u8]) -> Poll<Result<usize, Fault>> {
        let Some(uart) = self.active.as_mut() else {
            status::uart_fault(UartFault::Inactive);
            return Poll::Ready(Err(Fault::Communication));
        };
        poll_once(Read::read(uart, bytes)).map(|r| r.map_err(uart_error))
    }
    fn write(&mut self, bytes: &[u8]) -> Poll<Result<usize, Fault>> {
        let Some(uart) = self.active.as_mut() else {
            status::uart_fault(UartFault::Inactive);
            return Poll::Ready(Err(Fault::Communication));
        };
        poll_once(Write::write(uart, bytes)).map(|r| r.map_err(uart_error))
    }
    fn flush(&mut self) -> Poll<Result<(), Fault>> {
        let Some(uart) = self.active.as_mut() else {
            status::uart_fault(UartFault::Inactive);
            return Poll::Ready(Err(Fault::Communication));
        };
        poll_once(Write::flush(uart)).map(|r| r.map_err(uart_error))
    }
    fn busy(&self) -> bool {
        self.active.as_ref().is_some_and(BufferedUart::busy)
    }
    fn power_controlled(&self) -> bool {
        self.power.is_some()
    }
    fn power_on(&mut self) -> Result<(), Fault> {
        if self.active.is_some() {
            return Err(Fault::Communication);
        }
        self.power.as_mut().ok_or(Fault::Communication)?.set_low();
        status::supply_starting();
        Ok(())
    }
    fn enable_uart(&mut self) -> Result<(), Fault> {
        self.enable()
    }
    fn power_off(&mut self) -> Result<(), Fault> {
        if self.busy() {
            return Err(Fault::Communication);
        }
        self.disable();
        self.tx_bytes.fill(0);
        self.rx_bytes.fill(0);
        self.power.as_mut().ok_or(Fault::Communication)?.set_high();
        status::stage(Stage::Off);
        Ok(())
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
    fn failed(
        &mut self,
        request: rsk_device::fingerprint_protocol::Request,
        failure: rsk_fido::fingerprint::Failure,
    ) {
        status::driver_fault(request.command(), failure);
    }
}

pub type Bridge = Driver<Uart, Board>;

pub fn bridge(
    uart: Peri<'static, UART0>,
    tx: Peri<'static, PIN_0>,
    rx: Peri<'static, PIN_1>,
    power: Option<Output<'static>>,
) -> Bridge {
    static TX: StaticCell<[u8; TX_BYTES]> = StaticCell::new();
    static RX: StaticCell<[u8; RX_BYTES]> = StaticCell::new();
    let mut stream = Uart {
        active: None,
        uart,
        tx,
        rx,
        tx_bytes: TX.init([0; TX_BYTES]),
        rx_bytes: RX.init([0; RX_BYTES]),
        power,
    };
    if stream.power.is_some() {
        stream.disable();
    } else {
        stream.enable().expect("UART0 reset did not complete");
    }
    Driver::new(stream, Board)
}
