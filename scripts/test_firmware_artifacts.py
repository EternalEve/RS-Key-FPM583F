# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Exercise the build rows and packaging entry point that select firmware images."""

import os
import pathlib
import re
import shlex
import shutil
import subprocess
import tomllib

import pytest

import firmware_identity
import gate_lines

ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILD_ROWS = (
    "build firmware (release)",
    "build firmware (16M)",
    "build firmware (fingerprint)",
    "build firmware (fingerprint strict-config)",
    "build firmware (display)",
    "build firmware (test, --features no-touch)",
    "build firmware (fido-ccid)",
)
ELF = pathlib.Path("thumbv8m.main-none-eabihf/release/firmware")
DEFAULT = ROOT / "target" / ELF
NO_TOUCH = ROOT / "target/no-touch" / ELF
STRICT = ROOT / "target/strict-fingerprint" / ELF


def gate_row(name):
    source = (ROOT / "scripts/check.sh").read_text()
    rows = [body for _, body in gate_lines.logical_lines(source)
            if body.startswith(f'run "{name}" ')]
    assert len(rows) == 1, f"check.sh must run {name!r} exactly once"
    return rows[0]


def build_args(tmp_path, name):
    source = (ROOT / "scripts/check.sh").read_text()
    config = re.search(r"^FINGERPRINT_BUILD_ENV=\([\s\S]*?^\)", source, re.M)
    wrapper = re.search(r"^run\(\) \{[^\n]+", source, re.M)
    assert config and wrapper
    cargo = tmp_path / "cargo"
    cargo.write_text('#!/usr/bin/env bash\nprintf "arg=%s\\n" "$@"\n')
    cargo.chmod(0o755)
    done = subprocess.run(
        ["bash", "-c", "\n".join(("set -euo pipefail", config[0], wrapper[0], gate_row(name)))],
        capture_output=True, text=True,
        env={**os.environ, "PATH": f"{tmp_path}:{os.environ['PATH']}"},
    )
    assert done.returncode == 0, done.stdout + done.stderr
    return [line.removeprefix("arg=") for line in done.stdout.splitlines()
            if line.startswith("arg=")]


def test_gate_builds_have_distinct_output_directories(tmp_path):
    directories = {}
    for name in BUILD_ROWS:
        args = build_args(tmp_path, name)
        directories[name] = args[args.index("--target-dir") + 1] if "--target-dir" in args else "target"
    assert len(set(directories.values())) == len(BUILD_ROWS), directories


def package(elf, out, *args):
    return subprocess.run(["bash", str(ROOT / "scripts/pt.sh"), *args, str(elf), str(out)],
                          capture_output=True, text=True)


def identity_row(elf, name="firmware identity (release)"):
    argv = shlex.split(gate_row(name))[2:]
    expected = STRICT if name == "firmware identity (fingerprint strict-config)" else DEFAULT
    assert argv[-1] == str(expected.relative_to(ROOT))
    argv[-1] = str(elf)
    return subprocess.run(argv, cwd=ROOT, capture_output=True, text=True)


def test_default_image_packages_and_retains_its_marker(tmp_path):
    out = tmp_path / "firmware-pt.elf"
    done = package(DEFAULT, out)
    assert done.returncode == 0, done.stdout + done.stderr
    assert firmware_identity.read_marker(out) == firmware_identity.read_marker(DEFAULT)
    uf2 = tmp_path / "firmware.uf2"
    subprocess.run(["picotool", "uf2", "convert", str(out), "-t", "elf", str(uf2)],
                   check=True, capture_output=True)
    data = uf2.read_bytes()
    payload = b"".join(data[offset + 32:offset + 32 + int.from_bytes(data[offset + 16:offset + 20], "little")]
                       for offset in range(0, len(data), 512))
    flags, flash_size, kvmain = firmware_identity.read_marker(DEFAULT)
    assert firmware_identity.RECORD.pack(firmware_identity.MAGIC, firmware_identity.VERSION,
                                         flags, flash_size, kvmain) in payload


@pytest.mark.parametrize("name,function,directory", [
    ("firmware size budget", "firmware_size_budget", "target"),
    ("firmware stack floor", "firmware_stack_floor", "target"),
    ("no debug vendor command in the image", "debug_vendor_commands_absent", "target"),
    ("partition table fences the store", "partition_table_fences_the_store", "target"),
    ("sealed image retires its unsigned IMAGE_DEF", "release_image_retires_its_unsigned_image_def", "target"),
    ("partition table fences the store (16M)", "partition_table_fences_the_store", "target/16mb"),
    ("firmware stack floor (display)", "display_stack_floor", "target/display"),
])
def test_shell_audits_receive_their_profile_elf(name, function, directory):
    assert shlex.split(gate_row(name))[2:] == [function, str(pathlib.Path(directory) / ELF)]
    source = (ROOT / "scripts/check.sh").read_text()
    body = re.search(rf"^{function}\(\) \{{\n[\s\S]*?^\}}", source, re.M)
    assert body and re.search(r"(?:local )?elf=\$1\b", body[0]), body


@pytest.mark.parametrize("name", ["firmware-no-touch.elf", "renamed-release.elf"])
def test_no_touch_is_refused_by_the_packaging_entry_point(tmp_path, name):
    source = tmp_path / name
    shutil.copyfile(NO_TOUCH, source)
    out = tmp_path / "packaged.elf"
    done = package(source, out)
    assert done.returncode == 1, done.stdout + done.stderr
    assert "firmware-identity:" in done.stderr and "no-touch test image refused" in done.stderr
    assert not out.exists(), "identity rejection must precede partition-table output"


def test_missing_marker_is_refused_by_the_packaging_entry_point(tmp_path):
    source = tmp_path / "missing-marker.elf"
    subprocess.run(["arm-none-eabi-objcopy", "--remove-section=.rsk_build", str(DEFAULT), str(source)],
                   check=True, capture_output=True)
    done = package(source, tmp_path / "packaged.elf")
    assert done.returncode == 1, done.stdout + done.stderr
    assert "missing or malformed .rsk_build marker" in done.stderr


@pytest.mark.parametrize("source,policy,accepted", [
    (STRICT, "strict", True), (STRICT, "compat", False),
    (DEFAULT, "strict", False), (DEFAULT, "compat", True),
])
def test_packaging_checks_policy_inside_a_renamed_image(tmp_path, source, policy, accepted):
    renamed = tmp_path / "release.elf"
    shutil.copyfile(source, renamed)
    out = tmp_path / "packaged.elf"
    done = package(renamed, out, "--config-policy", policy)
    assert done.returncode == (0 if accepted else 1), done.stdout + done.stderr
    assert out.exists() == accepted
    if not accepted:
        assert "management-write policy does not match" in done.stderr


@pytest.mark.parametrize("field,value", [(0, b"UNKNOWN!"), (1, 2), (2, 32), (2, 6), (3, 0), (4, 0)])
def test_unrecognized_marker_is_refused_by_the_packaging_entry_point(tmp_path, field, value):
    values = list(firmware_identity.RECORD.unpack(subprocess.run(
        ["arm-none-eabi-objcopy", "-O", "binary", "--only-section=.rsk_build", str(DEFAULT), "/dev/stdout"],
        capture_output=True, check=True,
    ).stdout))
    values[field] = value
    marker = tmp_path / "marker.bin"
    marker.write_bytes(firmware_identity.RECORD.pack(*values))
    source = tmp_path / "unknown-marker.elf"
    subprocess.run(["arm-none-eabi-objcopy", f"--update-section=.rsk_build={marker}", str(DEFAULT), str(source)],
                   check=True, capture_output=True)
    done = package(source, tmp_path / "packaged.elf")
    assert done.returncode == 1, done.stdout + done.stderr
    assert "firmware-identity:" in done.stderr
    assert any(message in done.stderr for message in ("unrecognized .rsk_build marker",
               "incompatible fingerprint/display marker", "invalid flash geometry"))


def test_test_packaging_requires_explicit_matching_no_touch_policy(tmp_path):
    out = tmp_path / "firmware-no-touch.elf"
    done = package(NO_TOUCH, out, "--test-no-touch")
    assert done.returncode == 0, done.stdout + done.stderr
    assert firmware_identity.read_marker(out) == firmware_identity.read_marker(NO_TOUCH)
    done = package(DEFAULT, tmp_path / "mismatch.elf", "--test-no-touch")
    assert done.returncode == 1, done.stdout + done.stderr
    assert "expected a no-touch test image" in done.stderr


def test_actual_gate_row_rejects_no_touch_and_missing_markers(tmp_path):
    assert identity_row(DEFAULT).returncode == 0
    renamed = tmp_path / "firmware.elf"
    shutil.copyfile(NO_TOUCH, renamed)
    done = identity_row(renamed)
    assert done.returncode == 1 and "no-touch test image refused" in done.stderr, done
    subprocess.run(["arm-none-eabi-objcopy", "--remove-section=.rsk_build", str(DEFAULT), str(renamed)],
                   check=True, capture_output=True)
    done = identity_row(renamed)
    assert done.returncode == 1 and "missing or malformed .rsk_build marker" in done.stderr, done
    name = "firmware identity (fingerprint strict-config)"
    assert identity_row(STRICT, name).returncode == 0
    shutil.copyfile(DEFAULT, renamed)
    done = identity_row(renamed, name)
    assert done.returncode == 1 and "management-write policy does not match strict" in done.stderr, done


@pytest.mark.parametrize("directory,profile,geometry", [
    ("target", "default", (4 * 1024 * 1024, 1408 * 1024)),
    ("target/16mb", "default", (16 * 1024 * 1024, 1408 * 1024)),
    ("target/fingerprint", "fingerprint", (2 * 1024 * 1024, 896 * 1024)),
    ("target/strict-fingerprint", "fingerprint", (2 * 1024 * 1024, 896 * 1024)),
    ("target/display", "display", (16 * 1024 * 1024, 1408 * 1024)),
    ("target/no-touch", "no-touch", (4 * 1024 * 1024, 1408 * 1024)),
    ("target/fido-ccid", "fido-ccid", (4 * 1024 * 1024, 1408 * 1024)),
])
def test_gate_artifacts_match_their_build_configuration(directory, profile, geometry):
    flags, flash_size, kvmain = firmware_identity.read_marker(ROOT / directory / ELF)
    expected = firmware_identity.PROFILES[profile]
    if directory == "target/strict-fingerprint":
        expected |= firmware_identity.STRICT_CONFIG
    assert flags == expected
    assert (flash_size, kvmain) == geometry


def test_default_identity_row_refuses_an_opt_in_fido_ccid_image():
    done = identity_row(ROOT / "target/fido-ccid" / ELF)
    assert done.returncode == 1 and "do not match default" in done.stderr, done


@pytest.mark.parametrize("manifest,forward", [
    ("firmware/Cargo.toml", "rsk-device/fido-ccid"),
    ("tools/emu/Cargo.toml", "rsk-device/fido-ccid"),
    ("crates/rsk-device/Cargo.toml", "rsk-fido/fido-ccid"),
])
def test_fido_ccid_switch_is_opt_in_and_forwarded(manifest, forward):
    features = tomllib.loads((ROOT / manifest).read_text())["features"]
    pending = list(features.get("default", []))
    enabled = set()
    while pending:
        feature = pending.pop()
        if feature not in enabled:
            enabled.add(feature)
            pending.extend(features.get(feature, []))
    assert not {"fido-ccid", forward} & enabled, "the CCID FIDO switch must remain opt-in"
    assert forward in features["fido-ccid"], f"{manifest} did not forward the switch"


def test_nix_and_ci_use_checked_packaging_for_release_and_test_images():
    nix = (ROOT / "nix/firmware.nix").read_text()
    assert 'bash scripts/pt.sh ${lib.optionalString testNoTouch "--test-no-touch"}' in nix
    for name in ("firmware-no-touch", "firmware-no-touch-pqc", "firmware-no-touch-fips", "firmware-no-touch-fips-pqc"):
        assert re.search(rf'{name} = mkFirmware \{{\s*name = "{name}";\s*testNoTouch = true;', nix)
    ci = (ROOT / ".github/workflows/ci.yml").read_text()
    assert 'no-touch*) pt_args+=(--test-no-touch)' in ci
    assert 'target/${{ matrix.name }}/thumbv8m.main-none-eabihf/release/firmware' in ci
    release = (ROOT / ".github/workflows/release-build.yml").read_text()
    assert 'nix develop -c python scripts/firmware_identity.py --config-policy "$policy" "$out/$pkg.elf"' in release
    assert 'refusing to publish a no-touch firmware asset' in release
