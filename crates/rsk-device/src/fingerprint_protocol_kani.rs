// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;

#[kani::proof]
#[kani::unwind(25)]
fn capture_encoding_preserves_length_and_both_checksums() {
    let n: u8 = kani::any();
    kani::assume((1..=ENROLL_SAMPLES).contains(&n));
    let password: [u8; 4] = kani::any();
    let mut bytes = [0; 21];
    let length = Request::Capture(n).encode(password, &mut bytes).unwrap();
    assert_eq!(length, 19);
    assert_eq!(checksum(&bytes[..LINK_BYTES]), 0);
    assert_eq!(checksum(&bytes[LINK_BYTES..length]), 0);
    assert_eq!(&bytes[11..15], &password);
    assert_eq!(bytes[17], n);
}
