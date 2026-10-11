// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_PATH: AtomicUsize = AtomicUsize::new(0);

fn refusing_file() -> (FlashFile, PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "rsk-emu-persist-{}-{}",
        std::process::id(),
        NEXT_PATH.fetch_add(1, Ordering::Relaxed)
    ));
    let flash = FlashFile::open(Some(path.clone()), None).unwrap();
    fs::create_dir(&path).unwrap();
    (flash, path)
}

#[test]
fn file_write_failure_is_a_flash_write_failure() {
    let (mut flash, path) = refusing_file();
    let result = block_on(flash.write(0, &[0; <FlashFile as NorFlash>::WRITE_SIZE]));
    fs::remove_dir(path).unwrap();
    assert!(
        result.is_err(),
        "a refused file write must refuse the flash write"
    );
}

#[test]
fn file_write_failure_is_a_flash_erase_failure() {
    let (mut flash, path) = refusing_file();
    let result = block_on(flash.erase(0, <FlashFile as NorFlash>::ERASE_SIZE as u32));
    fs::remove_dir(path).unwrap();
    assert!(
        result.is_err(),
        "a refused file write must refuse the flash erase"
    );
}

#[test]
fn failed_persistence_poison_is_shared_with_the_other_partition() {
    let (mut flash, path) = refusing_file();
    let mut other = flash.clone();
    assert!(block_on(flash.write(0, &[0; <FlashFile as NorFlash>::WRITE_SIZE])).is_err());
    fs::remove_dir(path).unwrap();
    let mut data = [0; <FlashFile as ReadNorFlash>::READ_SIZE];
    assert!(block_on(other.read(0, &mut data)).is_err());
    assert!(block_on(other.erase(0, <FlashFile as NorFlash>::ERASE_SIZE as u32)).is_err());
}

#[test]
fn in_memory_flash_and_file_restart_still_work() {
    let mut memory = FlashFile::open(None, None).unwrap();
    let data = [0; <FlashFile as NorFlash>::WRITE_SIZE];
    block_on(memory.write(0, &data)).unwrap();
    let (mut file, path) = refusing_file();
    fs::remove_dir(&path).unwrap();
    block_on(file.write(0, &data)).unwrap();
    let mut reopened = FlashFile::open(Some(path.clone()), None).unwrap();
    let mut readback = [0xff; <FlashFile as NorFlash>::WRITE_SIZE];
    block_on(reopened.read(0, &mut readback)).unwrap();
    fs::remove_file(path).unwrap();
    assert_eq!(readback, data);
}
