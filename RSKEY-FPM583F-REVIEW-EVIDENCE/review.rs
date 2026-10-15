// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

pub use rsk_device::{fingerprint_driver, fingerprint_io, fingerprint_protocol};
mod fixture {
    include!(concat!(
        env!("RSK_REPO_ROOT"),
        "/crates/rsk-device/src/fingerprint_driver_fixture.rs"
    ));
}

use rsk_device::fingerprint_driver::{Driver, Runtime};
use rsk_device::presence::{Arbiter, SCOPE_FIDO, SCOPE_NONE};
use rsk_fido::fingerprint::Sensor;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Clone)]
struct BoardClock {
    now: Rc<Cell<u64>>,
    arbiter: Rc<Arbiter>,
}

impl Runtime for BoardClock {
    fn now_ms(&self) -> u64 {
        self.now.get()
    }
    fn cancelled(&self) -> bool {
        self.arbiter.cancel_requested()
    }
    fn wait(&mut self) {
        self.now.set(self.now.get() + 1);
    }
    fn processing(&mut self) {
        self.arbiter.set_cancellable(true);
        self.arbiter.set_up_pending(false);
    }
    fn waiting(&mut self, waiting: bool) {
        self.arbiter.set_up_pending(waiting);
    }
    fn end(&mut self) {
        self.arbiter.set_cancellable(false);
        self.arbiter.set_up_pending(false);
    }
}

#[test]
fn r1_cancel_is_retired_after_successful_drain_and_new_dispatch() {
    let module = Rc::new(RefCell::new(fixture::Module::default()));
    let clock = BoardClock {
        now: Rc::new(Cell::new(0)),
        arbiter: Rc::new(Arbiter::new()),
    };
    let mut driver = Driver::new(fixture::Wire(module.clone()), clock.clone());
    clock.arbiter.set_wait_scope(SCOPE_FIDO);
    assert_eq!(
        driver.prepare(30_000),
        Ok(()),
        "positive control: initial prepare"
    );
    let attempt = driver.start(1).unwrap();
    while module.borrow().calls(0x0121) == 0 {
        assert!(driver.poll().is_pending());
        driver.wait();
        assert!(clock.now.get() < 10_000);
    }
    clock.arbiter.request_cancel();
    driver.finish(attempt);
    for _ in 0..2_000 {
        driver.service_idle();
        if driver.ready() {
            break;
        }
        driver.wait();
    }
    assert!(driver.ready(), "old operation must really be drained");
    assert_eq!(module.borrow().calls(0x0115), 1);
    clock.arbiter.set_wait_scope(SCOPE_NONE);
    driver.release(false);
    clock.arbiter.set_wait_scope(SCOPE_FIDO);
    let actual = driver.prepare(clock.now.get() + 30_000);
    println!("R1: after a confirmed drain, a fresh dispatch returns {actual:?}");
    assert_eq!(actual, Ok(()));
    assert!(
        driver.ready(),
        "a fresh dispatch must retain the drained driver's availability"
    );
    clock.arbiter.request_cancel();
    assert_eq!(
        driver.prepare(clock.now.get() + 30_000),
        Err(rsk_fido::CtapError::KeepAliveCancel),
        "positive control: a fresh cancel must still cancel the current dispatch"
    );
}

struct Random(u64);
impl rsk_fido::Rng for Random {
    fn fill(&mut self, bytes: &mut [u8]) {
        for byte in bytes {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
            *byte = (self.0 >> 32) as u8;
        }
    }
}

fn pin_token_request(state: &rsk_fido::FidoState, pin: &[u8]) -> Vec<u8> {
    use rsk_crypto::pinproto::{self, PinProto};
    let scalar = [7; 32];
    let (x, y) = pinproto::public_xy(&scalar).unwrap();
    let (ax, ay) = state.ephemeral_public().unwrap();
    let mut shared = [0; 64];
    let n = pinproto::ecdh(PinProto::One, &scalar, &ax, &ay, &mut shared).unwrap();
    let mut encrypted = [0; 64];
    let k = pinproto::encrypt(
        PinProto::One,
        &shared[..n],
        &[0; 16],
        &rsk_crypto::sha256(pin)[..16],
        &mut encrypted,
    )
    .unwrap();
    let mut bytes = [0; 512];
    bytes[0] = rsk_fido::consts::CTAP_CLIENT_PIN;
    let mut e = minicbor::Encoder::new(minicbor::encode::write::Cursor::new(&mut bytes[1..]));
    e.map(4)
        .unwrap()
        .u8(1)
        .unwrap()
        .u8(1)
        .unwrap()
        .u8(2)
        .unwrap()
        .u8(5)
        .unwrap();
    e.u8(3)
        .unwrap()
        .map(5)
        .unwrap()
        .u8(1)
        .unwrap()
        .u8(2)
        .unwrap()
        .u8(3)
        .unwrap()
        .i8(-25)
        .unwrap()
        .i8(-1)
        .unwrap()
        .u8(1)
        .unwrap()
        .i8(-2)
        .unwrap()
        .bytes(&x)
        .unwrap()
        .i8(-3)
        .unwrap()
        .bytes(&y)
        .unwrap();
    e.u8(6).unwrap().bytes(&encrypted[..k]).unwrap();
    let len = e.writer().position() + 1;
    bytes[..len].to_vec()
}

#[test]
fn r2_corrupt_uv_state_preserves_correct_pin_and_its_budget() {
    use rsk_fido::consts::{EF_PIN, EF_UV_STATE, MAX_PIN_RETRIES};
    use rsk_fido::{AlwaysConfirm, Ctx, FidoState};
    use rsk_fs::{Fs, storage::ram::RamStorage};
    let dev = rsk_crypto::Device {
        serial_hash: &[0xab; 32],
        serial_id: &[1; 8],
        otp_key: None,
    };
    let mut fs = Fs::new(RamStorage::new());
    let mut rng = Random(44);
    let mut state = FidoState::new();
    let mut presence = AlwaysConfirm;
    let (mut sensor, _, _) = fixture::driver();
    let mut bindings = rsk_fido::bio_directory::Directory;
    let pin = b"836291";
    rsk_fido::seed::ensure_seed(&dev, &mut fs, &mut rng).unwrap();
    rsk_fido::clientpin::store_local_pin(&dev, &mut fs, pin).unwrap();
    rsk_fido::bio_directory::write_head(
        &mut fs,
        rsk_fido::bio_directory::Head {
            generation: 1,
            isolated: false,
        },
    )
    .unwrap();
    rsk_fido::bio_directory::write_entry(
        &mut fs,
        7,
        rsk_fido::bio_directory::Entry {
            stage: rsk_fido::bio_directory::Stage::Authorized,
            generation: 1,
            id: [0x37; rsk_fido::consts::BIO_ID_LEN],
            name: [0; rsk_fido::consts::BIO_NAME_MAX],
            name_len: 0,
        },
    )
    .unwrap();
    fs.put(EF_UV_STATE, &[1, 8, !8]).unwrap();
    state.ensure_initialized(&mut rng);
    let request = pin_token_request(&state, pin);
    let mut out = [0; 1024];
    let mut ctx = Ctx {
        dev,
        fs: &mut fs,
        rng: &mut rng,
        state: &mut state,
        now_ms: 0,
        presence: &mut presence,
        fingerprint: Some(rsk_fido::fingerprint::Context {
            sensor: &mut sensor,
            bindings: &mut bindings,
        }),
    };
    rsk_fido::process_cbor(&mut ctx, &request, &mut out);
    assert_eq!(
        out[0], 0,
        "positive control: the same correct PIN and ECDH request work"
    );
    ctx.fs.put(EF_UV_STATE, &[1, 8, 0]).unwrap();
    for i in 0..=MAX_PIN_RETRIES {
        rsk_fido::process_cbor(&mut ctx, &request, &mut out);
        let mut pin_record = [0; 35];
        ctx.fs.try_read(EF_PIN, &mut pin_record).unwrap();
        println!(
            "R2: correct PIN attempt {} returns 0x{:02x}, remaining PIN retries {}",
            i + 1,
            out[0],
            pin_record[0]
        );
        assert_eq!(out[0], 0);
        assert!(ctx.state.user_verified());
        assert!(!ctx.state.paut.fingerprint_verified);
        assert_eq!(pin_record[0], MAX_PIN_RETRIES);
    }
    let mut uv_record = [0; 3];
    assert_eq!(ctx.fs.try_read(EF_UV_STATE, &mut uv_record), Ok(Some(3)));
    assert_eq!(uv_record, [1, 8, 0]);
}

#[test]
fn r3_movement_feedback_aborts_the_sample_instead_of_allowing_a_retry() {
    use rsk_fido::bio_sensor::{self, Command, Feedback};
    for code in [0x0e, 0x11, 0x12] {
        let (mut driver, module, _) = fixture::driver();
        driver.prepare(30_000).unwrap();
        let capture = |first, remaining| Command::Capture {
            slot: 7,
            generation: 1,
            first,
            remaining,
        };
        let first = bio_sensor::execute(&mut driver, 1, capture(true, 6), 30_000).unwrap();
        assert!(matches!(
            first,
            bio_sensor::Result::Sample {
                remaining: 5,
                saved: false,
                ..
            }
        ));
        module.borrow_mut().quality.push_back(code);
        let result = bio_sensor::execute(&mut driver, 1, capture(false, 5), 30_000);
        println!("R3: capture completion 0x{code:02x} returns {result:?}");
        if code == 0x0e {
            assert_eq!(
                result,
                Ok(bio_sensor::Result::Sample {
                    feedback: Feedback::PoorQuality,
                    remaining: 5,
                    saved: false
                })
            );
            assert!(bio_sensor::execute(&mut driver, 1, capture(false, 5), 30_000).is_ok());
        } else {
            assert_eq!(result, Err(rsk_fido::CtapError::Other));
            assert_eq!(module.borrow().good_samples, 1);
            assert!(driver.ready(), "transport and hardware are still healthy");
            assert_eq!(
                bio_sensor::execute(&mut driver, 1, capture(false, 5), 30_000),
                Err(rsk_fido::CtapError::Other)
            );
            assert_eq!(
                module.borrow().calls(0x0111),
                2,
                "the continuation fails before a new capture is sent"
            );
        }
    }
}

#[test]
fn r5_empty_directory_bypasses_the_disabled_build_reset_guard() {
    use rsk_fido::consts::{CTAP_RESET, EF_BIO_DIRECTORY};
    use rsk_fido::{AlwaysConfirm, CtapError, Ctx, FidoState, bio_directory, bio_sensor};
    use rsk_fs::{Fs, storage::ram::RamStorage};
    let dev = rsk_crypto::Device {
        serial_hash: &[0xab; 32],
        serial_id: &[1; 8],
        otp_key: None,
    };
    let mut fs = Fs::new(RamStorage::new());
    let mut rng = Random(48);
    let mut state = FidoState::new();
    let mut presence = AlwaysConfirm;
    rsk_fido::seed::ensure_seed(&dev, &mut fs, &mut rng).unwrap();
    let mut inventory = bio_sensor::Inventory::default();
    inventory.set(99, true);
    bio_directory::reconcile(&mut fs, inventory, None).unwrap();
    assert!(bio_directory::read(&mut fs).unwrap().head.unwrap().isolated);
    let mut ctx = Ctx {
        dev,
        fs: &mut fs,
        rng: &mut rng,
        state: &mut state,
        now_ms: 0,
        presence: &mut presence,
        fingerprint: None,
    };
    let mut out = [0; 1024];
    rsk_fido::process_cbor(&mut ctx, &[CTAP_RESET], &mut out);
    assert_eq!(out[0], CtapError::NotAllowed.as_u8());
    assert!(bio_directory::read(ctx.fs).unwrap().head.unwrap().isolated);
    assert!(ctx.load_keydev().is_none());
    ctx.fs.put(EF_BIO_DIRECTORY, &[2]).unwrap();
    assert!(bio_directory::configuration_clean(ctx.fs).is_err());
    assert!(ctx.load_keydev().is_none());
    rsk_fido::process_cbor(&mut ctx, &[CTAP_RESET], &mut out);
    assert_eq!(out[0], CtapError::Other.as_u8());
    ctx.fs.put(EF_BIO_DIRECTORY, &[]).unwrap();
    assert_eq!(ctx.fs.try_read(EF_BIO_DIRECTORY, &mut []), Ok(Some(0)));
    assert!(bio_directory::read(ctx.fs).is_err());
    let clean = bio_directory::configuration_clean(ctx.fs);
    assert_eq!(clean, Ok(true));
    assert!(ctx.load_keydev().is_some());
    rsk_fido::process_cbor(&mut ctx, &[CTAP_RESET], &mut out);
    println!(
        "R5: empty directory yields configuration_clean={clean:?}, disabled-build Reset={:#04x}",
        out[0]
    );
    assert_eq!(out[0], 0);
    assert!(bio_directory::read(ctx.fs).unwrap().head.is_none());
}
