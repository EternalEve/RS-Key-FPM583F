// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;
use crate::consts::MAX_PIN_RETRIES;
use rsk_crypto::Device;
use rsk_fs::storage::faults::Cut;
use rsk_fs::storage::ram::RamStorage;

#[derive(Clone, Copy)]
enum Action {
    Match(u16),
    Fail(Failure, Matching),
    Cancel,
    Stale,
    Wait(Matching),
    ConfirmNoFinger,
}

struct Script {
    operations: Operations,
    action: Action,
    now: u64,
    cancelled: bool,
    starts: usize,
    finishes: usize,
}

impl Script {
    fn new(action: Action) -> Self {
        Self {
            operations: Operations::new(),
            action,
            now: 0,
            cancelled: false,
            starts: 0,
            finishes: 0,
        }
    }
}

impl Sensor for Script {
    fn now_ms(&self) -> u64 {
        self.now
    }
    fn ready(&self) -> bool {
        self.operations.state() == Lifecycle::Idle
    }
    fn start(&mut self, channel: u32) -> Result<Attempt, Failure> {
        self.starts += 1;
        let attempt = self.operations.begin(channel, self.now)?;
        if matches!(self.action, Action::ConfirmNoFinger) {
            self.operations.mark_matching(attempt, Matching::Unknown);
        }
        Ok(attempt)
    }
    fn poll(&mut self) -> Poll<Observation> {
        let Lifecycle::Active(mut attempt) = self.operations.state() else {
            panic!("no active attempt")
        };
        let result = match self.action {
            Action::Match(slot) => {
                self.operations.mark_matching(attempt, Matching::Started);
                Ok(Match {
                    slot,
                    interaction: true,
                })
            }
            Action::Fail(failure, matching) => {
                self.operations.mark_matching(attempt, matching);
                Err(failure)
            }
            Action::Cancel => {
                self.cancelled = true;
                return Poll::Pending;
            }
            Action::Stale => {
                attempt.id -= 1;
                Ok(Match {
                    slot: 7,
                    interaction: true,
                })
            }
            Action::Wait(matching) => {
                self.operations.mark_matching(attempt, matching);
                return Poll::Pending;
            }
            Action::ConfirmNoFinger => {
                self.operations
                    .mark_matching(attempt, Matching::NotAttempted);
                Err(Failure::InputTimeout)
            }
        };
        Poll::Ready(Observation { attempt, result })
    }
    fn cancelled(&self) -> bool {
        self.cancelled
    }
    fn matching(&self) -> Matching {
        self.operations.matching()
    }
    fn wait(&mut self) {
        self.now += 1000;
    }
    fn finish(&mut self, attempt: Attempt) {
        self.finishes += 1;
        self.operations.finish(attempt, self.now);
    }
    fn service_idle(&mut self) {
        self.operations.expire_drain(self.now);
    }
}

struct Directory {
    configured: bool,
    generation: u32,
    current_generation: u32,
}

impl Directory {
    fn active() -> Self {
        Self {
            configured: true,
            generation: 1,
            current_generation: 1,
        }
    }
}

impl<S: Storage> Bindings<S> for Directory {
    fn configuration(&mut self, _fs: &mut Fs<S>) -> Result<Option<u32>, CtapError> {
        Ok(self.configured.then_some(self.generation))
    }
    fn authorizes(
        &mut self,
        _fs: &mut Fs<S>,
        slot: u16,
        generation: u32,
    ) -> Result<bool, CtapError> {
        Ok(self.configured && slot == 7 && generation == self.current_generation)
    }
}

struct UnusedRng;
impl Rng for UnusedRng {
    fn fill(&mut self, _buf: &mut [u8]) {
        panic!("verification must not change key material")
    }
}

fn prepare<S: Storage>(fs: &mut Fs<S>) {
    let mut pin = [0; crate::clientpin::PIN_FILE_LEN];
    pin[0] = MAX_PIN_RETRIES;
    fs.put(EF_PIN, &pin).unwrap();
    store_retries(fs, MAX_UV_RETRIES).unwrap();
}

fn run<S: Storage>(
    fs: &mut Fs<S>,
    script: &mut Script,
    directory: &mut Directory,
    state: &mut crate::FidoState,
) -> Result<bool, CtapError> {
    let mut rng = UnusedRng;
    let mut presence = crate::AlwaysConfirm;
    let mut ctx = Ctx {
        dev: Device {
            serial_hash: &[0; 32],
            serial_id: &[0; 8],
            otp_key: None,
        },
        fs,
        rng: &mut rng,
        state,
        now_ms: 0,
        presence: &mut presence,
        fingerprint: Some(Context {
            sensor: script,
            bindings: directory,
        }),
    };
    verify(&mut ctx)
}

#[test]
fn only_a_current_authorized_match_verifies_without_touching_pin_lock() {
    let mut fs = Fs::new(RamStorage::new());
    prepare(&mut fs);
    let mut state = crate::FidoState::new();
    state.needs_power_cycle = true;
    state.new_pin_mismatches = 3;
    let mut script = Script::new(Action::Match(7));
    assert_eq!(
        run(&mut fs, &mut script, &mut Directory::active(), &mut state),
        Ok(true)
    );
    assert!(state.needs_power_cycle);
    assert_eq!(state.new_pin_mismatches, 3);
    assert_eq!(
        crate::clientpin::pin_retries_left(&mut fs),
        Some(MAX_PIN_RETRIES)
    );
    assert_eq!(retries(&mut fs), Ok(Some(MAX_UV_RETRIES)));
    assert_eq!((script.starts, script.finishes), (1, 1));
}

#[test]
fn failed_stale_and_unknown_matches_never_verify_or_refund() {
    for action in [
        Action::Fail(Failure::NoMatch, Matching::Started),
        Action::Match(9),
        Action::Stale,
        Action::Cancel,
    ] {
        let mut fs = Fs::new(RamStorage::new());
        prepare(&mut fs);
        let mut script = Script::new(action);
        let expected = if matches!(action, Action::Cancel) {
            CtapError::KeepAliveCancel
        } else {
            CtapError::UvInvalid
        };
        assert_eq!(
            run(
                &mut fs,
                &mut script,
                &mut Directory::active(),
                &mut crate::FidoState::new()
            ),
            Err(expected)
        );
        assert_eq!(retries(&mut fs), Ok(Some(MAX_UV_RETRIES - 1)));
        assert_eq!(script.finishes, 1);
    }
}

#[test]
fn a_changed_binding_generation_cannot_authorize_the_old_snapshot() {
    let mut fs = Fs::new(RamStorage::new());
    prepare(&mut fs);
    let mut directory = Directory::active();
    directory.current_generation += 1;
    assert_eq!(
        run(
            &mut fs,
            &mut Script::new(Action::Match(7)),
            &mut directory,
            &mut crate::FidoState::new()
        ),
        Err(CtapError::UvInvalid)
    );
    assert_eq!(retries(&mut fs), Ok(Some(MAX_UV_RETRIES - 1)));
}

#[test]
fn only_proven_input_timeout_refunds_the_persisted_attempt() {
    for matching in [Matching::NotStarted, Matching::Started, Matching::Unknown] {
        let mut fs = Fs::new(RamStorage::new());
        prepare(&mut fs);
        let mut script = Script::new(Action::Fail(Failure::InputTimeout, matching));
        assert_eq!(
            run(
                &mut fs,
                &mut script,
                &mut Directory::active(),
                &mut crate::FidoState::new()
            ),
            Err(CtapError::UserActionTimeout)
        );
        let expected = if matching == Matching::NotStarted {
            MAX_UV_RETRIES
        } else {
            MAX_UV_RETRIES - 1
        };
        let medium = fs.into_storage();
        assert_eq!(retries(&mut Fs::new(medium)), Ok(Some(expected)));
    }
}

#[test]
fn a_current_no_finger_result_can_resolve_an_unknown_matching_phase() {
    let mut fs = Fs::new(RamStorage::new());
    prepare(&mut fs);
    let mut script = Script::new(Action::ConfirmNoFinger);
    assert_eq!(
        run(
            &mut fs,
            &mut script,
            &mut Directory::active(),
            &mut crate::FidoState::new()
        ),
        Err(CtapError::UserActionTimeout)
    );
    assert_eq!(retries(&mut fs), Ok(Some(MAX_UV_RETRIES)));
}

#[test]
fn no_attempt_evidence_cannot_override_a_started_match_or_an_old_operation() {
    let mut operations = Operations::new();
    let attempt = operations.begin(1, 0).unwrap();
    operations.mark_matching(attempt, Matching::Started);
    operations.mark_matching(attempt, Matching::NotAttempted);
    assert_eq!(operations.matching(), Matching::Started);
    operations.mark_matching(attempt, Matching::Unknown);
    assert_eq!(operations.matching(), Matching::Started);
    operations.mark_matching(attempt, Matching::NotAttempted);
    assert_eq!(operations.matching(), Matching::Started);
    operations.finish(attempt, 1);
    assert!(operations.drained(attempt));
    let next = operations.begin(1, 2).unwrap();
    operations.mark_matching(next, Matching::Unknown);
    operations.mark_matching(attempt, Matching::NotAttempted);
    assert_eq!(operations.matching(), Matching::Unknown);
    operations.mark_matching(next, Matching::NotAttempted);
    assert_eq!(operations.matching(), Matching::NotAttempted);
}

#[test]
fn exhausted_budget_survives_reopen_and_needs_a_successful_pin_recovery() {
    let mut fs = Fs::new(RamStorage::new());
    prepare(&mut fs);
    store_retries(&mut fs, 1).unwrap();
    assert_eq!(
        run(
            &mut fs,
            &mut Script::new(Action::Fail(Failure::NoMatch, Matching::Started)),
            &mut Directory::active(),
            &mut crate::FidoState::new()
        ),
        Err(CtapError::UvBlocked)
    );
    let mut reopened = Fs::new(fs.into_storage());
    let mut script = Script::new(Action::Match(7));
    assert_eq!(
        run(
            &mut reopened,
            &mut script,
            &mut Directory::active(),
            &mut crate::FidoState::new()
        ),
        Err(CtapError::UvBlocked)
    );
    assert_eq!(script.starts, 0);
    restore_after_pin(&mut reopened).unwrap();
    assert_eq!(retries(&mut reopened), Ok(Some(MAX_UV_RETRIES)));
}

#[test]
fn pin_exhaustion_disables_fingerprint_but_a_temporary_lock_does_not() {
    let mut fs = Fs::new(RamStorage::new());
    prepare(&mut fs);
    let pin = [0; crate::clientpin::PIN_FILE_LEN];
    fs.put(EF_PIN, &pin).unwrap();
    let mut script = Script::new(Action::Match(7));
    assert_eq!(
        run(
            &mut fs,
            &mut script,
            &mut Directory::active(),
            &mut crate::FidoState::new()
        ),
        Err(CtapError::UvBlocked)
    );
    assert_eq!(retries(&mut fs), Ok(Some(0)));
    assert_eq!(script.starts, 0);
}

#[test]
fn refused_decrement_or_restore_cannot_verify_and_reopen_keeps_the_old_budget() {
    for writes in [0, 1] {
        let (storage, medium) = Cut::new();
        let mut fs = Fs::new(storage);
        prepare(&mut fs);
        medium.arm(writes);
        let mut script = Script::new(Action::Match(7));
        let mut state = crate::FidoState::new();
        assert_eq!(
            run(&mut fs, &mut script, &mut Directory::active(), &mut state),
            Err(CtapError::Other)
        );
        assert_eq!(script.starts, writes as usize);
        assert!(!state.user_verified());
        let mut reopened = Fs::new(fs.into_storage());
        assert_eq!(
            retries(&mut reopened),
            Ok(Some(MAX_UV_RETRIES - writes as u8))
        );
    }
}

#[test]
fn total_deadline_is_bounded_and_only_unmatched_waits_refund() {
    for matching in [Matching::NotStarted, Matching::Unknown] {
        let mut fs = Fs::new(RamStorage::new());
        prepare(&mut fs);
        let mut script = Script::new(Action::Wait(matching));
        assert_eq!(
            run(
                &mut fs,
                &mut script,
                &mut Directory::active(),
                &mut crate::FidoState::new()
            ),
            Err(CtapError::UserActionTimeout)
        );
        assert_eq!(script.now, VERIFY_TIMEOUT_MS);
        assert_eq!(script.finishes, 1);
        let expected = if matching == Matching::NotStarted {
            MAX_UV_RETRIES
        } else {
            MAX_UV_RETRIES - 1
        };
        assert_eq!(retries(&mut fs), Ok(Some(expected)));
    }
}

#[test]
fn missing_corrupt_and_unknown_state_cannot_initialize_under_an_authorized_binding() {
    for record in [
        None,
        Some(vec![STATE_VERSION, MAX_UV_RETRIES, 0]),
        Some(vec![STATE_VERSION + 1, MAX_UV_RETRIES, !MAX_UV_RETRIES]),
        Some(vec![STATE_VERSION, MAX_UV_RETRIES, !MAX_UV_RETRIES, 0]),
        Some(vec![]),
    ] {
        let mut fs = Fs::new(RamStorage::new());
        prepare(&mut fs);
        fs.force_delete(EF_UV_STATE).unwrap();
        if let Some(record) = record {
            fs.put(EF_UV_STATE, &record).unwrap();
        }
        assert_eq!(
            initialize_unconfigured(&mut fs, &mut Directory::active()),
            Err(CtapError::NotAllowed)
        );
        let mut script = Script::new(Action::Match(7));
        assert!(
            run(
                &mut fs,
                &mut script,
                &mut Directory::active(),
                &mut crate::FidoState::new()
            )
            .is_err()
        );
        assert_eq!(script.starts, 0);
    }
}

#[test]
fn lifecycle_separates_a_terminal_request_from_a_ready_module() {
    let mut operations = Operations::new();
    let attempt = operations.begin(7, 0).unwrap();
    assert!(operations.accepts(attempt, 1));
    assert!(!operations.accepts(attempt, attempt.deadline_ms));
    operations.finish(attempt, 1);
    assert!(!operations.accepts(attempt, 1));
    assert_eq!(operations.begin(8, 2), Err(Failure::Unavailable));
    let mut old = attempt;
    old.id -= 1;
    assert!(!operations.drained(old));
    assert!(operations.drained(attempt));
    let next = operations.begin(8, 2).unwrap();
    assert_ne!(next.id, attempt.id);
    assert!(!operations.accepts(attempt, 3));
    operations.finish(next, 3);
    operations.expire_drain(3 + DRAIN_TIMEOUT_MS);
    assert_eq!(operations.state(), Lifecycle::Unavailable);
    assert_eq!(
        operations.begin(8, 4 + DRAIN_TIMEOUT_MS),
        Err(Failure::Unavailable)
    );
}

#[test]
fn an_unknown_matching_phase_cannot_become_proven_unmatched_again() {
    let mut operations = Operations::new();
    let attempt = operations.begin(1, 0).unwrap();
    operations.mark_matching(attempt, Matching::Unknown);
    operations.mark_matching(attempt, Matching::NotStarted);
    assert_eq!(operations.matching(), Matching::Unknown);
}

#[test]
fn rolling_token_use_does_not_extend_fingerprint_up() {
    let mut state = crate::FidoState::new();
    state.begin_using_token(true, 1);
    state.mark_token_used(20_000);
    state.expire_user_presence(30_000);
    assert!(state.user_present());
    state.expire_user_presence(30_001);
    assert!(!state.user_present());
    assert!(state.user_verified());
}

#[test]
fn ctap_dispatch_expires_fingerprint_presence_without_retiring_a_used_token() {
    let mut fs = Fs::new(RamStorage::new());
    prepare(&mut fs);
    let mut script = Script::new(Action::Match(7));
    let mut directory = Directory::active();
    let mut state = crate::FidoState::new();
    assert_eq!(
        run(&mut fs, &mut script, &mut directory, &mut state),
        Ok(true)
    );
    script.now = 30_001;
    state.begin_using_token(true, 1);
    state.mark_token_used(20_000);
    let mut presence = crate::AlwaysConfirm;
    let mut rng = UnusedRng;
    let mut ctx = Ctx {
        dev: Device {
            serial_hash: &[0; 32],
            serial_id: &[0; 8],
            otp_key: None,
        },
        fs: &mut fs,
        rng: &mut rng,
        state: &mut state,
        now_ms: 30_001,
        presence: &mut presence,
        fingerprint: Some(Context {
            sensor: &mut script,
            bindings: &mut directory,
        }),
    };
    let mut out = [0; 1024];
    let n = crate::process_cbor(&mut ctx, &[crate::consts::CTAP_GET_INFO], &mut out);
    assert!(
        n > 1 && out[0] == 0,
        "the ordinary GetInfo request must succeed"
    );
    assert!(
        ctx.state.user_verified(),
        "rolling use must keep the token valid"
    );
    assert!(
        !ctx.state.user_present(),
        "expired fingerprint UP survived CTAP dispatch"
    );
    assert_eq!(
        script.starts, 1,
        "metadata must not run a second verification"
    );
}
