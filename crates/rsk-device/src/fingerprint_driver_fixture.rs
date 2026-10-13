// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::task::Poll;

use crate::fingerprint_driver::{Driver, Runtime};
use crate::fingerprint_io::{Fault, Stream};
use crate::fingerprint_protocol::HEADER;

pub(crate) fn response(command: u16, code: u32, data: &[u8]) -> Vec<u8> {
    let mut result = HEADER.to_vec();
    result.extend_from_slice(&((11 + data.len()) as u16).to_be_bytes());
    result.push(0u8.wrapping_sub(result.iter().copied().fold(0u8, u8::wrapping_add)));
    let mut app = vec![0; 4];
    app.extend_from_slice(&command.to_be_bytes());
    app.extend_from_slice(&code.to_be_bytes());
    app.extend_from_slice(data);
    app.push(0u8.wrapping_sub(app.iter().copied().fold(0u8, u8::wrapping_add)));
    result.extend(app);
    result
}

pub(crate) struct Module {
    pub tx: Vec<u8>,
    pub rx: VecDeque<u8>,
    pub commands: Vec<(u16, Vec<u8>)>,
    pub slots: [bool; 100],
    pub good_samples: u8,
    pub quality: VecDeque<u32>,
    pub busy_queries: usize,
    pub read_bytes: usize,
    pub write_bytes: usize,
    pub drop_command: Option<u16>,
    pub duplicate_command: Option<u16>,
    pub match_code: u32,
    pub match_slot: u16,
    pub save_code: u32,
    pub delete_code: u32,
    pub finger_present: bool,
    pub count_override: Option<u16>,
    pub corrupted: Option<u16>,
    pub active: u16,
    pub cancelled: bool,
    pub busy_wire: bool,
    pub controlled: bool,
    pub powered: bool,
    pub cycles: usize,
    pub cuts: usize,
}

impl Default for Module {
    fn default() -> Self {
        Self {
            tx: Vec::new(),
            rx: VecDeque::from([0x55]),
            commands: Vec::new(),
            slots: [false; 100],
            good_samples: 0,
            quality: VecDeque::new(),
            busy_queries: 0,
            read_bytes: 1,
            write_bytes: 1,
            drop_command: None,
            duplicate_command: None,
            match_code: 0,
            match_slot: 0,
            save_code: 0,
            delete_code: 0,
            finger_present: false,
            count_override: None,
            corrupted: None,
            active: 0,
            cancelled: false,
            busy_wire: false,
            controlled: false,
            powered: true,
            cycles: 0,
            cuts: 0,
        }
    }
}

impl Module {
    fn command(&mut self, packet: &[u8]) {
        assert_eq!(&packet[..8], &HEADER);
        assert_eq!(packet[..11].iter().copied().fold(0u8, u8::wrapping_add), 0);
        assert_eq!(packet[11..].iter().copied().fold(0u8, u8::wrapping_add), 0);
        assert_eq!(&packet[11..15], &[0; 4]);
        let command = u16::from_be_bytes([packet[15], packet[16]]);
        let params = &packet[17..packet.len() - 1];
        self.commands.push((command, params.to_vec()));
        let mut code = 0;
        let mut data = Vec::new();
        match command {
            0x020c => assert_eq!(params, &[0], "only ordinary sleep is supported"),
            0x0203 => data.extend_from_slice(
                &self
                    .count_override
                    .unwrap_or_else(|| self.slots.iter().filter(|s| **s).count() as u16)
                    .to_be_bytes(),
            ),
            0x0133 => {
                assert_eq!(params.len(), 2);
                let slot = u16::from_be_bytes([params[0], params[1]]) as usize;
                assert!(slot < self.slots.len());
                data.push(u8::from(self.slots[slot]));
                data.extend_from_slice(params);
            }
            0x0135 => data.push(u8::from(self.finger_present)),
            0x0111 => {
                assert_eq!(params.len(), 1);
                if params[0] == 1 {
                    self.good_samples = 0;
                }
                assert_eq!(params[0], self.good_samples + 1);
                self.active = command;
                self.cancelled = false;
            }
            0x0112 => {
                if self.cancelled {
                    code = 0x15;
                } else if self.busy_queries > 0 {
                    self.busy_queries -= 1;
                    code = 4;
                } else {
                    code = self.quality.pop_front().unwrap_or(0);
                    if code == 0 {
                        self.good_samples += 1;
                    }
                }
                // Deliberately not 100 on sample six: completion follows sample count.
                data.extend_from_slice(&[0, 0, self.good_samples * 10]);
            }
            0x0113 => {
                assert_eq!(self.good_samples, 6, "save must follow six real samples");
                assert_eq!(params.len(), 2);
                let slot = u16::from_be_bytes([params[0], params[1]]) as usize;
                assert!(!self.slots[slot], "never overwrite an occupied slot");
                if self.save_code == 0 {
                    self.slots[slot] = true;
                }
                self.active = slot as u16;
            }
            0x0114 => {
                code = self.save_code;
                data.extend_from_slice(&self.active.to_be_bytes());
            }
            0x0121 => {
                self.active = command;
                self.cancelled = false;
            }
            0x0122 => {
                code = if self.cancelled {
                    0x15
                } else {
                    self.match_code
                };
                if self.busy_queries > 0 {
                    self.busy_queries -= 1;
                    code = 4;
                }
                data.extend_from_slice(&[0, 1, 0x27, 0x0f]);
                data.extend_from_slice(&self.match_slot.to_be_bytes());
            }
            0x0115 => self.cancelled = true,
            0x0131 => {
                assert_eq!(params.len(), 3);
                if self.delete_code == 0 {
                    if params[0] == 1 {
                        self.slots.fill(false);
                    } else {
                        assert_eq!(params[0], 0);
                        let slot = u16::from_be_bytes([params[1], params[2]]) as usize;
                        self.slots[slot] = false;
                    }
                }
            }
            0x0132 => code = self.delete_code,
            _ => panic!("unexpected command {command:04x}"),
        }
        if self.drop_command == Some(command) {
            return;
        }
        if code == 4 {
            data.clear();
        }
        let mut reply = response(command, code, &data);
        if self.corrupted == Some(command) {
            *reply.last_mut().unwrap() ^= 1;
        }
        self.rx.extend(reply.iter());
        if self.duplicate_command == Some(command) {
            self.rx.extend(reply);
        }
    }

    pub fn calls(&self, command: u16) -> usize {
        self.commands.iter().filter(|(c, _)| *c == command).count()
    }
}

#[derive(Clone)]
pub(crate) struct Wire(pub Rc<RefCell<Module>>);

impl Stream for Wire {
    fn power_controlled(&self) -> bool {
        self.0.borrow().controlled
    }
    fn power_on(&mut self) -> Result<(), Fault> {
        let mut module = self.0.borrow_mut();
        assert!(!module.powered);
        assert!(module.tx.is_empty() && module.rx.is_empty());
        module.powered = true;
        module.cycles += 1;
        module.rx.push_back(0x55);
        Ok(())
    }
    fn power_off(&mut self) -> Result<(), Fault> {
        let mut module = self.0.borrow_mut();
        assert!(module.powered);
        assert!(module.tx.is_empty() && module.rx.is_empty());
        assert!(!module.busy_wire);
        assert_eq!(module.commands.last().unwrap().0, 0x020c);
        module.powered = false;
        module.cuts += 1;
        module.good_samples = 0;
        module.active = 0;
        Ok(())
    }
    fn read(&mut self, bytes: &mut [u8]) -> Poll<Result<usize, Fault>> {
        let mut module = self.0.borrow_mut();
        assert!(
            module.powered,
            "UART must not be used while the supply is off"
        );
        if module.rx.is_empty() {
            return Poll::Pending;
        }
        let n = bytes.len().min(module.rx.len()).min(module.read_bytes);
        for byte in &mut bytes[..n] {
            *byte = module.rx.pop_front().unwrap();
        }
        Poll::Ready(Ok(n))
    }
    fn write(&mut self, bytes: &[u8]) -> Poll<Result<usize, Fault>> {
        let mut module = self.0.borrow_mut();
        assert!(
            module.powered,
            "UART must not be used while the supply is off"
        );
        let n = bytes.len().min(module.write_bytes);
        module.tx.extend_from_slice(&bytes[..n]);
        if module.tx.len() >= 11 {
            let len = 11 + u16::from_be_bytes([module.tx[8], module.tx[9]]) as usize;
            if module.tx.len() == len {
                let packet = std::mem::take(&mut module.tx);
                module.command(&packet);
            }
        }
        Poll::Ready(Ok(n))
    }
    fn flush(&mut self) -> Poll<Result<(), Fault>> {
        Poll::Ready(Ok(()))
    }
    fn busy(&self) -> bool {
        self.0.borrow().busy_wire
    }
}

#[derive(Clone, Default)]
pub(crate) struct Clock(pub Rc<Cell<u64>>, pub Rc<Cell<bool>>);
impl Runtime for Clock {
    fn now_ms(&self) -> u64 {
        self.0.get()
    }
    fn cancelled(&self) -> bool {
        self.1.get()
    }
    fn wait(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

pub(crate) fn driver() -> (Driver<Wire, Clock>, Rc<RefCell<Module>>, Clock) {
    let module = Rc::new(RefCell::new(Module::default()));
    let clock = Clock::default();
    (
        Driver::new(Wire(module.clone()), clock.clone()),
        module,
        clock,
    )
}
