// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! CTAPHID over a TCP socket: the stream carries the same 64-byte reports the
//! USB HID interface would, in both directions, so a client is a `send(64)` /
//! `recv(64)` shim away from talking to the emulator.
//!
//! Framing and the native commands come from `rsk_usb::ctaphid` — the same
//! [`Reassembler`], [`TxFrames`] and command constants the firmware transport
//! uses. What is re-implemented here is the dispatch *arm order* (the async
//! `CtapHid::run` cannot drive a socket), so this is where CTAPHID behaviour can
//! drift from the device.

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rsk_usb::ctaphid::{
    CID_BROADCAST, CTAPHID_CANCEL, CTAPHID_CBOR, CTAPHID_ERROR, CTAPHID_INIT, CTAPHID_KEEPALIVE,
    CTAPHID_LOCK, CTAPHID_MSG, CTAPHID_PING, CTAPHID_SYNC, CTAPHID_UUID, CTAPHID_VENDOR_FIRST,
    CTAPHID_VERSION, CTAPHID_WINK, ChannelLock, CidAllocator, DEVICE_UUID, ERR_CHANNEL_BUSY,
    ERR_INVALID_CMD, ERR_INVALID_LEN, ERR_INVALID_PAR, ERR_MSG_TIMEOUT, HID_RPT_SIZE, Interruption,
    KEEPALIVE_MS, LOCK_MAX_SECONDS, Outcome, RX_TIMEOUT_MS, Reassembler, TxFrames, init_response,
    interruption, keepalive_status,
};

use crate::device::{Job, Jobs};
use crate::signals::Signals;

/// The emulator prints its wink to the terminal, so it really does have an
/// indicator to flash and may claim the capability.
const CAN_WINK: bool = true;

pub struct Shared {
    pub jobs: Jobs,
    pub signals: Arc<Signals>,
    /// Channel ids are device-wide, not per-connection: two client processes
    /// sharing one key must not be handed the same channel (§11.2.9.1.3).
    pub cids: Mutex<CidAllocator>,
    pub lock: Mutex<ChannelLock>,
    pub boot: Instant,
}

// Refines `RSKeyTransport!NoCrossChannelSplice` — SEC-TRANS-001.
// Refines `RSKeyTransport!NoSequenceGap` — SEC-TRANS-002.
// Refines `RSKeyTransport!NoBufferOverrun` — SEC-TRANS-003.
/// The emulator's dispatcher, and the SECOND consumer of the same `Reassembler`:
/// arm for arm what `CtapHid::on_frame` (`crates/rsk-usb/src/ctaphid.rs:668-681`)
/// does in the image, `lock.refuses` included — and the one `tests/*.py` runs.
pub fn serve(mut stream: TcpStream, shared: Arc<Shared>) -> io::Result<()> {
    let mut asm = Reassembler::new();
    let mut rx = FrameRx::default();
    loop {
        match rx.next(&stream, asm.in_progress())? {
            Rx::Eof => return Ok(()),
            // A host that sends frame 1 and walks away otherwise keeps the
            // reassembler for the life of the connection, and one connection is one
            // HID interface: every other channel is answered `CHANNEL_BUSY` until a
            // `CTAPHID_INIT` on the abandoned one. `CtapHid::run` recovers by racing
            // its read against the same deadline.
            Rx::Timeout => {
                let cid = asm.current_cid();
                asm.abort();
                write_msg(&mut stream, cid, CTAPHID_ERROR, &[ERR_MSG_TIMEOUT])?;
            }
            Rx::Frame => {
                let now_ms = shared.boot.elapsed().as_millis() as u64;
                match asm.feed(&rx.frame) {
                    Outcome::None => {}
                    Outcome::Error(cid, code) => {
                        write_msg(&mut stream, cid, CTAPHID_ERROR, &[code])?
                    }
                    Outcome::Message(cid, cmd)
                        if shared.lock.lock().unwrap().refuses(cid, cmd, now_ms) =>
                    {
                        write_msg(&mut stream, cid, CTAPHID_ERROR, &[ERR_CHANNEL_BUSY])?
                    }
                    Outcome::Message(cid, cmd) => {
                        dispatch(&mut stream, &shared, &mut asm, &mut rx, cid, cmd, now_ms)?
                    }
                }
            }
        }
    }
}

/// What [`FrameRx::next`] came back with.
enum Rx {
    /// A whole 64-byte report is in [`FrameRx::frame`].
    Frame,
    /// The deadline passed with the report still incomplete.
    Timeout,
    /// The peer closed.
    Eof,
}

/// The RX half of the connection: whole 64-byte reports, deadlined while a
/// multi-frame message is mid-reassembly.
///
/// The bytes of a half-arrived report are carried across the deadline rather than
/// dropped — TCP may split a report, and half a frame discarded here misaligns
/// every frame behind it. The device transport needs none of that: USB delivers
/// whole reports, so its `select` can simply abandon the read.
struct FrameRx {
    frame: [u8; HID_RPT_SIZE],
    have: usize,
}

impl Default for FrameRx {
    fn default() -> Self {
        Self {
            frame: [0; HID_RPT_SIZE],
            have: 0,
        }
    }
}

impl FrameRx {
    /// The next whole report. `bounded` is "a message is mid-reassembly", and only
    /// then is the wait finite — an idle transport waits for its next client as
    /// long as it takes, exactly as `CtapHid::run` does.
    fn next(&mut self, mut stream: &TcpStream, bounded: bool) -> io::Result<Rx> {
        let deadline = Instant::now() + Duration::from_millis(RX_TIMEOUT_MS);
        loop {
            let left = if bounded {
                let left = deadline.saturating_duration_since(Instant::now());
                // A zero read timeout means "no timeout" to the socket layer.
                if left.is_zero() {
                    return Ok(Rx::Timeout);
                }
                Some(left)
            } else {
                None
            };
            stream.set_read_timeout(left)?;
            match stream.read(&mut self.frame[self.have..]) {
                Ok(0) => return Ok(Rx::Eof),
                Ok(n) => {
                    self.have += n;
                    if self.have == HID_RPT_SIZE {
                        self.have = 0;
                        return Ok(Rx::Frame);
                    }
                }
                // An expired read timeout arrives as one or the other by platform.
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    return Ok(Rx::Timeout);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }
}

fn dispatch(
    stream: &mut TcpStream,
    shared: &Arc<Shared>,
    asm: &mut Reassembler,
    rx: &mut FrameRx,
    cid: u32,
    cmd: u8,
    now_ms: u64,
) -> io::Result<()> {
    match cmd {
        CTAPHID_INIT => {
            // A fresh session drops anything selected over MSG, so U2F (which has
            // no SELECT) cannot inherit it. Queued and not awaited, because the
            // board's is a `MSG_DESELECT` store the transport never waits on: one
            // FIFO still runs it before the next MSG, and an INIT that waited would
            // sit behind an open on-panel modal for the whole inactivity bound.
            let (done, _) = mpsc::channel();
            let _ = shared.jobs.send(Job::DeselectMsg, done);
            // nonce(8) | newcid(4) | iface | major | minor | build | caps
            let nonce = asm.message();
            let assigned = if cid == CID_BROADCAST {
                shared.cids.lock().unwrap().allocate()
            } else {
                cid
            };
            let resp = init_response(nonce, assigned, CAN_WINK);
            write_msg(stream, cid, CTAPHID_INIT, &resp)
        }
        CTAPHID_PING | CTAPHID_SYNC => {
            let echo = asm.message().to_vec();
            write_msg(stream, cid, cmd, &echo)
        }
        CTAPHID_WINK => {
            eprintln!("emu: ✨ wink");
            write_msg(stream, cid, CTAPHID_WINK, &[])
        }
        CTAPHID_LOCK => match asm.message() {
            [secs] if *secs <= LOCK_MAX_SECONDS => {
                shared.lock.lock().unwrap().arm(cid, *secs, now_ms);
                write_msg(stream, cid, CTAPHID_LOCK, &[])
            }
            [_] => write_msg(stream, cid, CTAPHID_ERROR, &[ERR_INVALID_PAR]),
            _ => write_msg(stream, cid, CTAPHID_ERROR, &[ERR_INVALID_LEN]),
        },
        CTAPHID_VERSION => {
            let v = rsk_sdk::FIRMWARE_VERSION;
            write_msg(stream, cid, CTAPHID_VERSION, &[v.0, v.1, v.2, 0])
        }
        CTAPHID_UUID => write_msg(stream, cid, CTAPHID_UUID, &DEVICE_UUID),
        // Never acknowledged; it aborts an in-flight touch on this channel only.
        CTAPHID_CANCEL => {
            shared.signals.request_cancel(cid);
            Ok(())
        }
        CTAPHID_MSG => {
            let data = asm.message().to_vec();
            let out = run_job(shared, Job::Msg { cid, data }, false, stream, rx, cid)?;
            write_job_reply(stream, cid, CTAPHID_MSG, out)
        }
        CTAPHID_CBOR => {
            let data = asm.message().to_vec();
            if data.is_empty() {
                return write_msg(stream, cid, CTAPHID_ERROR, &[ERR_INVALID_LEN]);
            }
            let out = run_job(shared, Job::Cbor { cid, data }, true, stream, rx, cid)?;
            write_job_reply(stream, cid, CTAPHID_CBOR, out)
        }
        cmd if cmd >= CTAPHID_VENDOR_FIRST => {
            let data = asm.message().to_vec();
            let job = Job::Vendor {
                cmd: cmd & !0x80,
                data,
            };
            let out = run_job(shared, job, false, stream, rx, cid)?;
            write_job_reply(stream, cid, cmd, out)
        }
        _ => write_msg(stream, cid, CTAPHID_ERROR, &[ERR_INVALID_CMD]),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum JobReply {
    Response(Option<Vec<u8>>),
    Resynchronized([u8; rsk_usb::ctaphid::INIT_NONCE_LEN]),
}

fn write_job_reply(stream: &mut TcpStream, cid: u32, cmd: u8, reply: JobReply) -> io::Result<()> {
    match reply {
        JobReply::Response(Some(body)) => write_msg(stream, cid, cmd, &body),
        JobReply::Response(None) => write_msg(stream, cid, CTAPHID_ERROR, &[ERR_INVALID_CMD]),
        JobReply::Resynchronized(nonce) => write_msg(
            stream,
            cid,
            CTAPHID_INIT,
            &init_response(&nonce, cid, CAN_WINK),
        ),
    }
}

fn run_job(
    shared: &Arc<Shared>,
    job: Job,
    is_cbor: bool,
    stream: &mut TcpStream,
    rx: &mut FrameRx,
    cid: u32,
) -> io::Result<JobReply> {
    let (tx, answer) = mpsc::channel();
    if shared.jobs.send(job, tx).is_err() {
        return Err(io::Error::other("the device thread is gone"));
    }
    let mut resync = None;
    loop {
        match answer.recv_timeout(Duration::from_millis(KEEPALIVE_MS)) {
            Ok(out) => {
                return Ok(match resync {
                    Some(nonce) => JobReply::Resynchronized(nonce),
                    None => JobReply::Response(out),
                });
            }
            Err(RecvTimeoutError::Timeout) => {
                let up = shared.signals.up_pending_for(crate::signals::SCOPE_FIDO);
                if let Some(status) = keepalive_status(is_cbor, up) {
                    write_msg(stream, cid, CTAPHID_KEEPALIVE, &[status])?;
                }
                // Keep matching cancellable without advertising a touch wait.
                if up || shared.signals.cancellable_for(crate::signals::SCOPE_FIDO) {
                    match poll_watch(stream, rx, cid)? {
                        Interruption::Cancel => shared.signals.request_cancel(cid),
                        Interruption::Resync(nonce) => {
                            // Drain the old job before replying to INIT, so its
                            // eventual response cannot become the next command's.
                            resync = Some(nonce);
                            shared.signals.request_cancel(cid);
                            let (done, _) = mpsc::channel();
                            let _ = shared.jobs.send(Job::DeselectMsg, done);
                        }
                        Interruption::Error(other, code) => {
                            write_msg(stream, other, CTAPHID_ERROR, &[code])?
                        }
                        Interruption::None => {}
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err(io::Error::other("the device thread dropped the job"));
            }
        }
    }
}

// An interrupt watch and the next dispatch share partial reports: discarding
// bytes at the end of a job would misalign the connection's subsequent reports.
impl FrameRx {
    /// One report per poll; incomplete bytes remain for the next dispatch too.
    fn poll<R: Read>(&mut self, mut src: R, cid: u32) -> io::Result<Interruption> {
        loop {
            match src.read(&mut self.frame[self.have..]) {
                // The peer closed. `serve`'s own read reports that; here it just
                // means no cancel is coming.
                Ok(0) => return Ok(Interruption::None),
                Ok(n) => {
                    self.have += n;
                    if self.have < HID_RPT_SIZE {
                        continue;
                    }
                    self.have = 0;
                    return Ok(interruption(&self.frame, HID_RPT_SIZE, cid));
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(Interruption::None),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
    }
}

/// [`FrameRx::poll`] over the connection, which has to leave blocking mode for
/// it and be back in it before `serve` reads the next frame.
fn poll_watch(stream: &TcpStream, watch: &mut FrameRx, cid: u32) -> io::Result<Interruption> {
    stream.set_nonblocking(true)?;
    let seen = watch.poll(stream, cid);
    stream.set_nonblocking(false)?;
    seen
}

fn write_msg(stream: &mut TcpStream, cid: u32, cmd: u8, data: &[u8]) -> io::Result<()> {
    for f in TxFrames::new(cid, cmd, data) {
        stream.write_all(&f)?;
    }
    stream.flush()
}

#[cfg(test)]
#[path = "hid_tests.rs"]
mod tests;
