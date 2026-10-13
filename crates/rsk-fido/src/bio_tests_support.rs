// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use crate::bio_directory::{self, Directory};
use crate::bio_sensor::{self, Capabilities, Command, Feedback, Inventory};
use crate::consts::*;
use crate::fingerprint::{
    Attempt, Failure, Lifecycle, Match, Matching, Observation, Operations, Sensor,
};
use crate::{AlwaysConfirm, CtapError, Ctx, FidoState, Rng};
use core::task::Poll;
use minicbor::encode::Write;
use rsk_crypto::{
    Device,
    pinproto::{self, PinProto},
};
use rsk_fs::{Fs, Storage};

pub struct Random(pub u64);
impl Rng for Random {
    fn fill(&mut self, out: &mut [u8]) {
        for b in out {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
            *b = (self.0 >> 32) as u8;
        }
    }
}

pub struct Module {
    pub inventory: Inventory,
    pub now: u64,
    pub commands: Vec<Command>,
    pub feedback: Feedback,
    pub fail: bool,
    pub unknown: bool,
    pub cancel: bool,
    pub stale: bool,
    pub wait: bool,
    pub offline: bool,
    pub samples: u8,
    pub match_slot: u16,
    pub startup_ms: u64,
    pub holds: Vec<bool>,
    pub prepares: usize,
    operations: Operations,
    command: Option<Command>,
}
impl Default for Module {
    fn default() -> Self {
        Self {
            inventory: Inventory::default(),
            now: 10,
            commands: Vec::new(),
            feedback: Feedback::Good,
            fail: false,
            unknown: false,
            cancel: false,
            stale: false,
            wait: false,
            offline: false,
            samples: 3,
            match_slot: 0,
            startup_ms: 0,
            holds: Vec::new(),
            prepares: 0,
            operations: Operations::new(),
            command: None,
        }
    }
}
impl Sensor for Module {
    fn prepare(&mut self, deadline_ms: u64) -> Result<(), CtapError> {
        self.prepares += 1;
        self.now = self.now.saturating_add(self.startup_ms);
        self.startup_ms = 0;
        self.service_idle();
        if self.now >= deadline_ms {
            Err(CtapError::UserActionTimeout)
        } else if self.cancelled() {
            Err(CtapError::KeepAliveCancel)
        } else if self.ready() {
            Ok(())
        } else {
            Err(CtapError::Other)
        }
    }
    fn release(&mut self, enrollment: bool) {
        self.holds.push(enrollment);
    }
    fn now_ms(&self) -> u64 {
        self.now
    }
    fn ready(&self) -> bool {
        !self.offline && self.operations.state() == Lifecycle::Idle
    }
    fn start(&mut self, channel: u32) -> Result<Attempt, Failure> {
        self.command = None;
        self.operations.begin(channel, self.now)
    }
    fn poll(&mut self) -> Poll<Observation> {
        let Lifecycle::Active(attempt) = self.operations.state() else {
            return Poll::Pending;
        };
        Poll::Ready(Observation {
            attempt,
            result: Ok(Match {
                slot: self.match_slot,
                interaction: true,
            }),
        })
    }
    fn cancelled(&self) -> bool {
        self.cancel
    }
    fn matching(&self) -> Matching {
        Matching::Started
    }
    fn wait(&mut self) {
        self.now += 1;
    }
    fn finish(&mut self, attempt: Attempt) {
        self.operations.finish(attempt, self.now);
    }
    fn service_idle(&mut self) {
        if let Lifecycle::Draining { attempt, .. } = self.operations.state() {
            self.operations.drained(attempt);
        }
    }
    fn capabilities(&self) -> Option<Capabilities> {
        Some(Capabilities {
            samples: self.samples,
            slots: bio_sensor::MODULE_SLOTS as u16,
        })
    }
    fn start_management(&mut self, channel: u32, command: Command) -> Result<Attempt, Failure> {
        self.command = Some(command);
        self.commands.push(command);
        self.operations.begin(channel, self.now)
    }
    fn poll_management(&mut self) -> Poll<bio_sensor::Observation> {
        let Lifecycle::Active(mut attempt) = self.operations.state() else {
            return Poll::Pending;
        };
        let command = self.command.unwrap();
        if command != Command::Inventory && self.wait {
            return Poll::Pending;
        }
        if self.stale {
            attempt.id = attempt.id.wrapping_sub(1);
        }
        if self.fail && command != Command::Inventory {
            return Poll::Ready(bio_sensor::Observation {
                attempt,
                result: Err(Failure::Communication),
            });
        }
        let result = match command {
            Command::Inventory => bio_sensor::Result::Inventory(self.inventory),
            Command::Capture {
                slot, remaining, ..
            } => {
                let remaining = if self.feedback == Feedback::Good {
                    remaining - 1
                } else {
                    remaining
                };
                if remaining == 0 {
                    self.inventory.set(slot, true);
                }
                bio_sensor::Result::Sample {
                    feedback: self.feedback,
                    remaining,
                    saved: remaining == 0,
                }
            }
            Command::Delete { slot } => {
                self.inventory.set(slot, false);
                bio_sensor::Result::Deleted
            }
            Command::Clear => {
                self.inventory = Inventory::default();
                bio_sensor::Result::Cleared
            }
        };
        Poll::Ready(bio_sensor::Observation {
            attempt,
            result: if self.unknown && command != Command::Inventory {
                Err(Failure::Communication)
            } else {
                Ok(result)
            },
        })
    }
}

pub struct Environment<S: Storage> {
    pub fs: Fs<S>,
    pub module: Module,
    pub state: FidoState,
    rng: Random,
    directory: Directory,
    presence: AlwaysConfirm,
}
impl<S: Storage> Environment<S> {
    pub fn provisioned(storage: S) -> Self {
        let mut fs = Fs::new(storage);
        fs.put(EF_PIN, &[MAX_PIN_RETRIES; crate::clientpin::PIN_FILE_LEN])
            .unwrap();
        let mut rng = Random(4);
        crate::seed::ensure_seed(&Self::dev(), &mut fs, &mut rng).unwrap();
        let mut env = Self {
            fs,
            module: Module::default(),
            state: FidoState::new(),
            rng,
            directory: Directory,
            presence: AlwaysConfirm,
        };
        env.authorize();
        env
    }
    pub fn authorize(&mut self) {
        self.state.paut.token = [0x53; 32];
        self.state.begin_using_token(false, self.module.now);
        self.state.paut.permissions = crate::state::PERM_BE;
    }
    pub fn dev() -> Device<'static> {
        Device {
            serial_hash: &[0xAB; 32],
            serial_id: &[1; 8],
            otp_key: None,
        }
    }
    pub fn ctx(&mut self) -> Ctx<'_, S, Random> {
        // The zero-sized binding/presence objects live through this borrowed context.
        Ctx {
            dev: Device {
                serial_hash: &[0xAB; 32],
                serial_id: &[1; 8],
                otp_key: None,
            },
            fs: &mut self.fs,
            rng: &mut self.rng,
            state: &mut self.state,
            now_ms: self.module.now,
            presence: &mut self.presence,
            fingerprint: Some(crate::fingerprint::Context {
                sensor: &mut self.module,
                bindings: &mut self.directory,
            }),
        }
    }
    pub fn exchange(&mut self, command: u8, id: Option<&[u8]>, name: Option<&str>) -> Vec<u8> {
        let mut params = [0; MAX_RAW_SUBPARA];
        let mut p = minicbor::Encoder::new(minicbor::encode::write::Cursor::new(&mut params[..]));
        p.map(u64::from(id.is_some()) + u64::from(name.is_some()))
            .unwrap();
        if let Some(id) = id {
            p.u8(1).unwrap().bytes(id).unwrap();
        }
        if let Some(name) = name {
            p.u8(2).unwrap().str(name).unwrap();
        }
        let n = if id.is_some() || name.is_some() {
            p.writer().position()
        } else {
            0
        };
        let mut message = vec![1, command];
        message.extend_from_slice(&params[..n]);
        let mut auth = [0; 32];
        pinproto::authenticate(PinProto::Two, &self.state.paut.token, &message, &mut auth).unwrap();
        let mut data = [0; 512];
        data[0] = CTAP_BIO_ENROLLMENT;
        let mut enc = minicbor::Encoder::new(minicbor::encode::write::Cursor::new(&mut data[1..]));
        enc.map(4 + u64::from(n != 0))
            .unwrap()
            .u8(1)
            .unwrap()
            .u8(1)
            .unwrap()
            .u8(2)
            .unwrap()
            .u8(command)
            .unwrap();
        if n != 0 {
            enc.u8(3).unwrap();
            enc.writer_mut().write_all(&params[..n]).unwrap();
        }
        enc.u8(4)
            .unwrap()
            .u8(2)
            .unwrap()
            .u8(5)
            .unwrap()
            .bytes(&auth)
            .unwrap();
        let count = enc.writer().position() + 1;
        let mut out = vec![0; MAX_MSG_SIZE as usize];
        let written = crate::process_cbor(&mut self.ctx(), &data[..count], &mut out);
        out.truncate(written);
        out
    }
    pub fn begin(&mut self) -> [u8; BIO_ID_LEN] {
        let response = self.exchange(1, None, None);
        assert_eq!(response[0], 0, "begin: {response:?}");
        let mut d = minicbor::Decoder::new(&response[1..]);
        let mut id = [0; BIO_ID_LEN];
        for _ in 0..d.map().unwrap().unwrap() {
            if d.u8().unwrap() == 4 {
                id.copy_from_slice(d.bytes().unwrap());
            } else {
                d.skip().unwrap();
            }
        }
        id
    }
    pub fn enroll(&mut self) -> [u8; BIO_ID_LEN] {
        let id = self.begin();
        for _ in 1..self.module.samples {
            assert_eq!(self.exchange(2, Some(&id), None)[0], 0);
        }
        id
    }
    pub fn reopen(mut self) -> Self {
        self.fs = Fs::new(self.fs.into_storage());
        self.fs.scan();
        self.state = FidoState::new();
        self.module.service_idle();
        self.authorize();
        self
    }
    pub fn configured(&mut self) -> Result<Option<u32>, CtapError> {
        use crate::fingerprint::Bindings;
        Directory.configuration(&mut self.fs)
    }
    pub fn reconcile(&mut self) -> Result<(), CtapError> {
        crate::bio_enrollment::refresh(&mut self.ctx())
    }
    pub fn reset(&mut self) -> u8 {
        let mut out = [0; 64];
        crate::process_cbor(&mut self.ctx(), &[CTAP_RESET], &mut out);
        out[0]
    }
    pub fn snapshot(&mut self) -> bio_directory::Snapshot {
        bio_directory::read(&mut self.fs).unwrap()
    }
}
