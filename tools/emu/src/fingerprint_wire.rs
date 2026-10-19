// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Synthetic module below the production driver; requests are independently decoded.

use std::cell::Cell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Poll;
use std::time::{Duration, Instant};

use rsk_device::fingerprint_driver::{Driver, Runtime};
use rsk_device::fingerprint_io::{Fault, SERVICE_INTERVAL_MS, Stream};
use rsk_fido::CtapError;
use rsk_fido::bio_sensor::{Inventory, MODULE_SLOTS};
use rsk_fido::fingerprint::Sensor;

use crate::signals::Signals;

const HEADER: [u8; 8] = [0xf1, 0x1f, 0xe2, 0x2e, 0xb6, 0x6b, 0xa8, 0x8a];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Good,
    Poor,
    MovementLarge,
    MovementSmall,
    NoMatch,
    NoFinger,
    FingerPresent,
    Wait,
    SaveLost,
    DeleteLost,
    Corrupt,
    Duplicate,
    Unknown,
    NoLift,
    SleepLost,
    BootLost,
}

impl Mode {
    fn read(path: &PathBuf) -> Result<Self, Fault> {
        let text = std::fs::read_to_string(path).map_err(|_| Fault::Communication)?;
        match text.trim() {
            "good" => Ok(Self::Good),
            "poor" => Ok(Self::Poor),
            "movement-large" => Ok(Self::MovementLarge),
            "movement-small" => Ok(Self::MovementSmall),
            "nomatch" => Ok(Self::NoMatch),
            "nofinger" => Ok(Self::NoFinger),
            "present" => Ok(Self::FingerPresent),
            "wait" => Ok(Self::Wait),
            "save-lost" => Ok(Self::SaveLost),
            "delete-lost" => Ok(Self::DeleteLost),
            "corrupt" => Ok(Self::Corrupt),
            "duplicate" => Ok(Self::Duplicate),
            "unknown" => Ok(Self::Unknown),
            "no-lift" => Ok(Self::NoLift),
            "sleep-lost" => Ok(Self::SleepLost),
            "boot-lost" => Ok(Self::BootLost),
            _ => Err(Fault::Communication),
        }
    }
}

pub struct Clock {
    attach: Rc<Cell<Instant>>,
    signals: Arc<Signals>,
}

impl Runtime for Clock {
    fn now_ms(&self) -> u64 {
        self.attach.get().elapsed().as_millis() as u64
    }
    fn cancelled(&self) -> bool {
        self.signals.cancelled()
    }
    fn wait(&mut self) {
        std::thread::sleep(Duration::from_millis(SERVICE_INTERVAL_MS));
    }
    fn processing(&mut self) {
        self.signals.set_cancellable(true);
        self.signals.set_up_pending(false);
    }
    fn end(&mut self) {
        self.signals.set_cancellable(false);
        self.signals.set_up_pending(false);
    }
    fn waiting(&mut self, waiting: bool) {
        self.signals.set_up_pending(waiting);
    }
}

pub struct Wire {
    controlled: bool,
    powered: bool,
    asleep: bool,
    path: PathBuf,
    inventory_path: PathBuf,
    rx: VecDeque<u8>,
    tx: Vec<u8>,
    samples: u8,
    requested: u8,
    sample_result: Option<(u32, u8)>,
    save_slot: u16,
    interrupted: bool,
}

impl Wire {
    fn read_inventory(&self) -> Result<Inventory, Fault> {
        let bytes = std::fs::read(&self.inventory_path).map_err(|_| Fault::Communication)?;
        let inventory = Inventory(bytes.try_into().map_err(|_| Fault::Communication)?);
        if inventory.0[inventory.0.len() - 1] & 0xf0 != 0 {
            return Err(Fault::Communication);
        }
        Ok(inventory)
    }

    fn persist(&self, inventory: Inventory) -> Result<(), Fault> {
        std::fs::write(&self.inventory_path, inventory.0).map_err(|_| Fault::Communication)
    }

    fn command(&mut self, packet: &[u8]) -> Result<(), Fault> {
        if packet.len() < 18
            || packet[..8] != HEADER
            || packet[..11].iter().copied().fold(0u8, u8::wrapping_add) != 0
            || packet[11..].iter().copied().fold(0u8, u8::wrapping_add) != 0
            || packet[11..15] != [0; 4]
        {
            return Err(Fault::Communication);
        }
        let command = u16::from_be_bytes([packet[15], packet[16]]);
        let params = &packet[17..packet.len() - 1];
        eprintln!("fpm-wire: command {command:04x}");
        let mode = Mode::read(&self.path)?;
        let mut code = 0u32;
        let mut data = Vec::new();
        let slot = |bytes: &[u8]| -> Result<u16, Fault> {
            let bytes: [u8; 2] = bytes.try_into().map_err(|_| Fault::Communication)?;
            let slot = u16::from_be_bytes(bytes);
            if slot >= MODULE_SLOTS as u16 {
                Err(Fault::Communication)
            } else {
                Ok(slot)
            }
        };
        match command {
            0x020c if params == [0] => {
                if mode == Mode::SleepLost {
                    return Ok(());
                }
                self.asleep = true;
            }
            0x0203 if params.is_empty() => {
                if mode == Mode::BootLost {
                    return Ok(());
                }
                let inventory = self.read_inventory()?;
                let count = (0..MODULE_SLOTS as u16)
                    .filter(|s| inventory.contains(*s))
                    .count() as u16;
                data.extend_from_slice(&count.to_be_bytes());
            }
            0x0133 => {
                let slot = slot(params)?;
                data.push(u8::from(self.read_inventory()?.contains(slot)));
                data.extend_from_slice(&slot.to_be_bytes());
            }
            0x0135 if params.is_empty() => {
                data.push(u8::from(matches!(mode, Mode::NoLift | Mode::FingerPresent)));
            }
            0x0111 if params.len() == 1 && (1..=6).contains(&params[0]) => {
                if params[0] == 1 {
                    self.samples = 0;
                }
                if params[0] != self.samples + 1 {
                    return Err(Fault::Communication);
                }
                self.requested = params[0];
                self.sample_result = None;
                self.interrupted = false;
            }
            0x0112 if params.is_empty() => {
                if self.interrupted {
                    code = 0x15;
                } else if mode == Mode::Wait {
                    code = 4;
                } else {
                    let result = *self.sample_result.get_or_insert_with(|| {
                        let code = match mode {
                            Mode::Poor => 0x0e,
                            Mode::MovementLarge => 0x11,
                            Mode::MovementSmall => 0x12,
                            Mode::NoFinger => 8,
                            _ => 0,
                        };
                        if code == 0 {
                            self.samples = self.requested;
                        }
                        (code, self.samples * 10)
                    });
                    code = result.0;
                    data.extend_from_slice(&[0, 0, result.1]);
                }
            }
            0x0113 => {
                self.save_slot = slot(params)?;
                if self.samples != 6 {
                    return Err(Fault::Communication);
                }
                let mut inventory = self.read_inventory()?;
                if inventory.contains(self.save_slot) {
                    code = 0x13;
                } else {
                    inventory.set(self.save_slot, true);
                    self.persist(inventory)?;
                }
            }
            0x0114 if params.is_empty() => {
                if mode == Mode::SaveLost {
                    return Ok(());
                }
                data.extend_from_slice(&self.save_slot.to_be_bytes());
            }
            0x0121 if params.is_empty() => self.interrupted = false,
            0x0122 if params.is_empty() => {
                if self.interrupted {
                    code = 0x15;
                } else {
                    code = match mode {
                        Mode::NoFinger => 8,
                        Mode::Wait => 4,
                        _ => 0,
                    };
                    if code == 0 {
                        let inventory = self.read_inventory()?;
                        let slot = if mode == Mode::Unknown {
                            99
                        } else {
                            (0..MODULE_SLOTS as u16)
                                .find(|s| inventory.contains(*s))
                                .unwrap_or(0)
                        };
                        let matched = mode != Mode::NoMatch && !inventory.empty();
                        data.extend_from_slice(&[0, u8::from(matched), 0x27, 0x0f]);
                        data.extend_from_slice(&slot.to_be_bytes());
                    }
                }
            }
            0x0115 if params.is_empty() => {
                self.interrupted = true;
                self.sample_result = None;
            }
            0x0131 if params.len() == 3 && params[0] <= 1 => {
                let mut inventory = self.read_inventory()?;
                if params[0] == 1 {
                    inventory = Inventory::default();
                } else {
                    inventory.set(slot(&params[1..])?, false);
                }
                self.persist(inventory)?;
            }
            0x0132 if params.is_empty() => {
                if mode == Mode::DeleteLost {
                    return Ok(());
                }
            }
            _ => return Err(Fault::Communication),
        }
        if code != 0 {
            data.clear();
        }
        let mut response = HEADER.to_vec();
        response.extend_from_slice(&((11 + data.len()) as u16).to_be_bytes());
        response.push(0u8.wrapping_sub(response.iter().copied().fold(0u8, u8::wrapping_add)));
        let mut app = vec![0; 4];
        app.extend_from_slice(&command.to_be_bytes());
        app.extend_from_slice(&code.to_be_bytes());
        app.extend(data);
        app.push(0u8.wrapping_sub(app.iter().copied().fold(0u8, u8::wrapping_add)));
        response.extend(app);
        if command == 0x0122 && mode == Mode::Corrupt {
            *response.last_mut().ok_or(Fault::Communication)? ^= 1;
        }
        self.rx.extend(&response);
        if command == 0x0122 && mode == Mode::Duplicate {
            self.rx.extend(response);
        }
        Ok(())
    }
}

impl Stream for Wire {
    fn power_controlled(&self) -> bool {
        self.controlled
    }
    fn power_on(&mut self) -> Result<(), Fault> {
        if self.powered || !self.rx.is_empty() || !self.tx.is_empty() {
            return Err(Fault::Communication);
        }
        self.powered = true;
        self.asleep = false;
        self.rx.push_back(0x55);
        eprintln!("fpm-power: on");
        Ok(())
    }
    fn power_off(&mut self) -> Result<(), Fault> {
        if !self.powered || !self.asleep || !self.rx.is_empty() || !self.tx.is_empty() {
            return Err(Fault::Communication);
        }
        self.powered = false;
        self.samples = 0;
        self.requested = 0;
        self.sample_result = None;
        self.save_slot = 0;
        self.interrupted = false;
        eprintln!("fpm-power: off");
        Ok(())
    }
    fn read(&mut self, bytes: &mut [u8]) -> Poll<Result<usize, Fault>> {
        if !self.powered {
            return Poll::Ready(Err(Fault::Communication));
        }
        if self.rx.is_empty() {
            return Poll::Pending;
        }
        let n = bytes.len().min(self.rx.len()).min(5);
        for byte in &mut bytes[..n] {
            *byte = self.rx.pop_front().expect("bounded above");
        }
        Poll::Ready(Ok(n))
    }
    fn write(&mut self, bytes: &[u8]) -> Poll<Result<usize, Fault>> {
        if !self.powered || self.asleep {
            return Poll::Ready(Err(Fault::Communication));
        }
        let n = bytes.len().min(7);
        self.tx.extend_from_slice(&bytes[..n]);
        if self.tx.len() >= 11 {
            let len = 11 + u16::from_be_bytes([self.tx[8], self.tx[9]]) as usize;
            if !(18..=21).contains(&len) || self.tx.len() > len {
                return Poll::Ready(Err(Fault::Communication));
            }
            if self.tx.len() == len {
                let packet = std::mem::take(&mut self.tx);
                if let Err(fault) = self.command(&packet) {
                    return Poll::Ready(Err(fault));
                }
            }
        }
        Poll::Ready(Ok(n))
    }
    fn flush(&mut self) -> Poll<Result<(), Fault>> {
        Poll::Ready(Ok(()))
    }
    fn busy(&self) -> bool {
        false
    }
}

pub struct Fixture {
    pub driver: Driver<Wire, Clock>,
    controlled: bool,
    path: PathBuf,
    attach: Rc<Cell<Instant>>,
    signals: Arc<Signals>,
}

impl Fixture {
    pub fn new(
        path: PathBuf,
        attach: Rc<Cell<Instant>>,
        signals: Arc<Signals>,
        controlled: bool,
    ) -> Result<Self, CtapError> {
        Mode::read(&path).map_err(|_| CtapError::Other)?;
        let wire = Wire {
            controlled,
            powered: !controlled,
            asleep: false,
            inventory_path: path.with_extension("inventory"),
            path: path.clone(),
            rx: if controlled {
                VecDeque::new()
            } else {
                VecDeque::from([0x55])
            },
            tx: Vec::new(),
            samples: 0,
            requested: 0,
            sample_result: None,
            save_slot: 0,
            interrupted: false,
        };
        wire.read_inventory().map_err(|_| CtapError::Other)?;
        let clock = Clock {
            attach: attach.clone(),
            signals: signals.clone(),
        };
        let mut driver = Driver::new(wire, clock);
        let deadline = driver.now_ms() + 3_100;
        while !controlled && !driver.ready() && driver.now_ms() < deadline {
            driver.service_idle();
            if driver.diagnostic().is_some() {
                return Err(CtapError::Other);
            }
            driver.wait();
        }
        if !controlled && !driver.ready() {
            return Err(CtapError::Other);
        }
        Ok(Self {
            driver,
            controlled,
            path,
            attach,
            signals,
        })
    }

    pub fn reset(&mut self) -> Result<(), CtapError> {
        *self = Self::new(
            self.path.clone(),
            self.attach.clone(),
            self.signals.clone(),
            self.controlled,
        )?;
        Ok(())
    }
}
