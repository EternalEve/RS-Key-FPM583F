// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;

const CTAP_WRITE_CONFIG: u8 = 0x43;
const OTP_SET_DEVICE_INFO: u8 = 0x15;
const INS_WRITE_CONFIG: u8 = 0x1C;
const TAG_USB_ENABLED: u8 = 0x03;
const ENABLED_CAPS: u16 = rsk_devconf::CAP_FIDO2 | rsk_devconf::CAP_U2F | rsk_devconf::CAP_OTP;

fn config_blob(mask: u16) -> Vec<u8> {
    let mask = mask.to_be_bytes();
    vec![4, TAG_USB_ENABLED, 2, mask[0], mask[1]]
}

fn assert_enabled(jobs: &Jobs, path: &Path, expected: u16) {
    ask(jobs, Job::OtpStatus);
    assert_eq!(
        rsk_devconf::read_enabled_caps(&mut mount(path)),
        expected,
        "the persisted application mask"
    );
    ask(jobs, Job::Replug(Unplug::Operator));
    let readback = ask(
        jobs,
        Job::Vendor {
            cmd: CTAP_READ_CONFIG,
            data: Vec::new(),
        },
    );
    let mut tlvs = &readback[1..];
    loop {
        assert!(tlvs.len() >= 2, "READ CONFIG omitted USB_ENABLED");
        let (tag, len) = (tlvs[0], tlvs[1] as usize);
        assert!(tlvs.len() >= 2 + len, "truncated DeviceInfo TLV");
        if tag == TAG_USB_ENABLED {
            assert_eq!(&tlvs[2..2 + len], expected.to_be_bytes());
            break;
        }
        tlvs = &tlvs[2 + len..];
    }
}

#[test]
fn raw_ctaphid_config_write_follows_the_selected_profile() {
    let (path, jobs, _signals, device) = bench("admin-ctaphid");
    let answer = queue(
        &jobs,
        Job::Vendor {
            cmd: CTAP_WRITE_CONFIG,
            data: config_blob(ENABLED_CAPS),
        },
    )
    .recv_timeout(Duration::from_secs(5))
    .expect("the management command answered");
    if cfg!(feature = "strict-config") {
        assert!(answer.is_none(), "strict CTAPHID accepted an ungated write");
        assert_enabled(&jobs, &path, rsk_devconf::SUPPORTED_CAPS);
    } else {
        assert_eq!(
            answer,
            Some(Vec::new()),
            "the compatibility write was refused"
        );
        assert_enabled(&jobs, &path, ENABLED_CAPS);
    }
    shut_down(path, jobs, device);
}

#[test]
fn otp_device_info_write_follows_the_selected_profile() {
    let (path, jobs, _signals, device) = bench("admin-otp");
    let before = rsk_otp::hid::status_frame(ask(&jobs, Job::OtpStatus).try_into().unwrap());
    let answer = ask(
        &jobs,
        Job::OtpHid {
            slot: OTP_SET_DEVICE_INFO,
            payload: config_blob(ENABLED_CAPS),
        },
    );
    assert_eq!(answer.len(), rsk_otp::hid::REPORT_SIZE, "no response body");
    let sequence = |status: &[u8]| status[4];
    if cfg!(feature = "strict-config") {
        assert_eq!(
            sequence(&answer),
            sequence(&before),
            "a no-op advanced pgmSeq"
        );
        assert_enabled(&jobs, &path, rsk_devconf::SUPPORTED_CAPS);
    } else {
        assert_eq!(sequence(&answer), sequence(&before).wrapping_add(1));
        assert_enabled(&jobs, &path, ENABLED_CAPS);
    }
    shut_down(path, jobs, device);
}

fn write_caps(jobs: &Jobs, mask: u16) {
    let aid = rsk_mgmt::MANAGEMENT_AID;
    let mut select = vec![0, 0xA4, 4, 0, aid.len() as u8];
    select.extend_from_slice(aid);
    let selected = ask(jobs, Job::Apdu(select));
    assert_eq!(&selected[selected.len() - 2..], &SW_OK);
    let blob = config_blob(mask);
    let mut command = vec![0, INS_WRITE_CONFIG, 0, 0, blob.len() as u8];
    command.extend_from_slice(&blob);
    assert_eq!(ask(jobs, Job::Apdu(command)), SW_OK);
}

#[test]
fn confirmed_ccid_config_write_remains_available() {
    let (path, jobs, _signals, device) = bench("admin-ccid");
    write_caps(&jobs, ENABLED_CAPS);
    assert_enabled(&jobs, &path, ENABLED_CAPS);
    shut_down(path, jobs, device);
}

#[test]
fn disabled_hid_applications_stay_closed_until_ccid_reenables_them() {
    for disabled in [rsk_devconf::CAP_FIDO2, rsk_devconf::CAP_U2F] {
        let (path, jobs, _signals, device) = bench(&format!("admin-disabled-{disabled}"));
        let info = || {
            ask(
                &jobs,
                Job::Cbor {
                    cid: CID,
                    data: vec![rsk_fido::consts::CTAP_GET_INFO],
                },
            )
        };
        let version = || {
            ask(
                &jobs,
                Job::Msg {
                    cid: CID,
                    data: U2F_VERSION.to_vec(),
                },
            )
        };
        write_caps(&jobs, rsk_devconf::SUPPORTED_CAPS & !disabled);
        for _ in 0..2 {
            if disabled == rsk_devconf::CAP_FIDO2 {
                assert_eq!(
                    info(),
                    vec![rsk_fido::CtapError::OperationDenied as u8],
                    "disabled FIDO2 answered HID CBOR"
                );
                assert_eq!(version(), U2F_V2_OK);
            } else {
                assert_eq!(info()[0], 0, "FIDO2 remains enabled");
                assert_eq!(
                    version(),
                    SW_CONDITIONS_NOT_SATISFIED,
                    "disabled U2F answered HID MSG"
                );
            }
            ask(
                &jobs,
                Job::Vendor {
                    cmd: CTAP_READ_CONFIG,
                    data: Vec::new(),
                },
            );
            ask(&jobs, Job::Replug(Unplug::Operator));
        }
        write_caps(&jobs, rsk_devconf::SUPPORTED_CAPS);
        assert_eq!(info()[0], 0, "CCID restored FIDO2");
        assert_eq!(version(), U2F_V2_OK, "CCID restored U2F");
        shut_down(path, jobs, device);
    }
}
