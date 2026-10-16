# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Run actual gate rows with recording Cargo to protect fingerprint CI wiring.
Real compiler and client fault controls are kept in the R4 review evidence.
"""

import os
import pathlib
import re
import shlex
import subprocess
import textwrap

import pytest

import gate_lines

ROOT = pathlib.Path(__file__).resolve().parents[1]


def row(name):
    source = (ROOT / "scripts/check.sh").read_text()
    matches = [body for _, body in gate_lines.logical_lines(source)
               if body.startswith((f'run "{name}" ', f'run_tests "{name}" '))]
    assert len(matches) == 1, f"check.sh must run {name!r} exactly once"
    return matches[0]


def run_row(tmp_path, name, status):
    source = (ROOT / "scripts/check.sh").read_text()
    config = re.search(r"^FINGERPRINT_BUILD_ENV=\([\s\S]*?^\)", source, re.M)
    wrapper = re.search(r"^run\(\) \{[^\n]+", source, re.M)
    assert config, "fingerprint candidate configuration is missing"
    assert wrapper, "the gate's run wrapper is missing"
    cargo = tmp_path / "cargo"
    cargo.write_text(
        '#!/usr/bin/env bash\n'
        'printf "arg=%s\\n" "$@"\n'
        'printf "FLASH_SIZE=%s\\nKVMAIN=%s\\nFINGERPRINT_TOUCH_PIN=%s\\nFINGERPRINT_POWER_PIN=%s\\n" '
        '"$FLASH_SIZE" "$KVMAIN" "$FINGERPRINT_TOUCH_PIN" "$FINGERPRINT_POWER_PIN"\n'
        'echo "R4 injected cargo failure" >&2\n'
        f'exit {status}\n'
    )
    cargo.chmod(0o755)
    script = '\n'.join(("set -euo pipefail", config[0], wrapper[0], row(name),
                        'echo "row completed"'))
    env = {key: value for key, value in os.environ.items()
           if key not in {"FLASH_SIZE", "KVMAIN", "FINGERPRINT_TOUCH_PIN", "FINGERPRINT_POWER_PIN"}}
    return subprocess.run(["bash", "-c", script], text=True, capture_output=True,
                          env={**env, "PATH": f"{tmp_path}:{os.environ['PATH']}",
                               "HOST": "x86_64-unknown-linux-gnu"})


def biometric_job():
    source = (ROOT / ".github/workflows/emulator.yml").read_text()
    found = re.search(r"^  biometrics:\n[\s\S]*?(?=^  [\w-]+:|\Z)", source, re.M)
    assert found, "the independent biometric CI job is missing"
    return found[0]


def biometric_command():
    found = re.search(r"^        run: \|\n((?:          [^\n]*\n)+)", biometric_job(), re.M)
    assert found, "the biometric CI runner is missing"
    return textwrap.dedent(found[1])


def run_biometric_command(tmp_path, runner_status, signal=False, tee_status=0):
    nix = tmp_path / "nix"
    nix.write_text('#!/usr/bin/env bash\necho "R4 synthetic client result"\n'
                   + ('kill -TERM "$PPID"\n' if signal else '')
                   + f'exit {runner_status}\n')
    nix.chmod(0o755)
    if tee_status:
        tee = tmp_path / "tee"
        tee.write_text(f'#!/usr/bin/env bash\ncat > "$1"\nexit {tee_status}\n')
        tee.chmod(0o755)
    done = subprocess.run(["bash", "-e", "-o", "pipefail", "-c", biometric_command()],
                          cwd=tmp_path, text=True, capture_output=True,
                          env={**os.environ, "PATH": f"{tmp_path}:{os.environ['PATH']}"})
    logs = tmp_path / "target/bio-client-ci"
    return done, (logs / "status.txt").read_text(), (logs / "runner.log").read_text()


@pytest.mark.parametrize("verb", ["clippy", "build"])
def test_candidate_rows_compile_the_real_fingerprint_configuration(tmp_path, verb):
    name = "clippy (fingerprint firmware)" if verb == "clippy" else "build firmware (fingerprint)"
    done = run_row(tmp_path, name, 0)
    assert done.returncode == 0, done.stdout + done.stderr
    args = [line.removeprefix("arg=") for line in done.stdout.splitlines()
            if line.startswith("arg=")]
    assert args == [verb, "--release", "-p", "firmware", "--target",
                    "thumbv8m.main-none-eabihf", "--features", "fingerprint",
                    *(["--target-dir", "target/fingerprint"] if verb == "build" else []),
                    *(["--", "-D", "warnings"] if verb == "clippy" else [])]
    for setting in ("FLASH_SIZE=2M", "KVMAIN=896K", "FINGERPRINT_TOUCH_PIN=2",
                    "FINGERPRINT_POWER_PIN=3"):
        assert setting in done.stdout.splitlines()


@pytest.mark.parametrize("name", ["clippy (fingerprint firmware)", "build firmware (fingerprint)"])
def test_candidate_row_failure_stops_the_gate(tmp_path, name):
    done = run_row(tmp_path, name, 47)
    assert done.returncode == 47, done.stdout + done.stderr
    assert "R4 injected cargo failure" in done.stderr
    assert "row completed" not in done.stdout


def test_the_no_touch_image_still_follows_the_candidate_build():
    source = (ROOT / "scripts/check.sh").read_text()
    assert source.index(row("build firmware (fingerprint)")) < source.index(
        row("build firmware (test, --features no-touch)"))


@pytest.mark.parametrize("feature", ["fingerprint-fixture", "bio-fixture-2mb"])
@pytest.mark.parametrize("verb", ["clippy", "test"])
def test_gate_checks_both_emulator_fixture_configurations(feature, verb):
    args = shlex.split(row(f"{verb} (emu {feature})"))[2:]
    assert args[:2] == ["cargo", verb]
    assert args[args.index("--manifest-path") + 1] == "tools/emu/Cargo.toml"
    assert args[args.index("--features") + 1] == feature
    assert args[args.index("--target") + 1] == "$HOST"
    if verb == "clippy":
        assert "--all-targets" in args
        assert args[-3:] == ["--", "-D", "warnings"]
    else:
        assert row(f"{verb} (emu {feature})").startswith("run_tests ")


@pytest.mark.parametrize("client", ["standard", "fingerprint"])
def test_emulator_ci_runs_the_complete_short_client_suites(client):
    source = (ROOT / ".github/workflows/emulator.yml").read_text()
    job = re.search(r"^  sockets:\n[\s\S]*?(?=^  [\w-]+:|\Z)", source, re.M)[0]
    assert "needs: changes" in job
    assert "if: needs.changes.outputs.emulator == 'true'" in job
    commands = [body.removeprefix("run: ") for body, live in gate_lines.yaml_runs(job) if live
                and f"scripts/{client}-client.sh" in body]
    assert commands == [f"nix develop -c ./scripts/{client}-client.sh"]
    assert "continue-on-error" not in job
    step = next(part for part in re.split(r"^      - ", job, flags=re.M)
                if f"scripts/{client}-client.sh" in part)
    assert "        if:" not in step


@pytest.mark.parametrize("path", ["scripts/standard-client.sh", "scripts/fingerprint-client.sh",
                                  "scripts/ci-scope.sh", "tests/standard_client/env.nix",
                                  "firmware/src/fingerprint.rs"])
def test_client_inputs_enable_emulator_ci(path):
    done = subprocess.run(["bash", "scripts/ci-scope.sh"], cwd=ROOT,
                          input=path + "\n", capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    assert "emulator=true" in done.stdout.splitlines(), done.stdout


def test_biometric_ci_is_independent_and_keeps_diagnostics():
    job = biometric_job()
    source = (ROOT / ".github/workflows/emulator.yml").read_text()
    for trigger in ("  pull_request:", "  push:", "  schedule:"):
        assert trigger in source.splitlines()
    assert "needs: changes" in job
    assert "if: needs.changes.outputs.emulator == 'true'" in job
    assert "timeout-minutes: 60" in job and "timeout-minutes: 50" in job
    assert "continue-on-error" not in job
    assert "nix develop -c ./scripts/bio-client.sh 2>&1 | tee" in biometric_command()
    assert "-k " not in biometric_command()
    step = job[job.index("      - name: full biometric client matrix"):
               job.index("      - name: biometric runner diagnostics")]
    assert "        if:" not in step
    upload = job[job.index("      - name: biometric runner diagnostics"):]
    assert "if: always()" in upload
    assert "uses: actions/upload-artifact@" in upload
    assert "path: target/bio-client-ci/" in upload


@pytest.mark.parametrize("status", [0, 1, 143])
def test_biometric_ci_preserves_runner_failure_and_output(tmp_path, status):
    done, metadata, output = run_biometric_command(tmp_path, status)
    assert done.returncode == status, done.stdout + done.stderr
    assert f"runner_status={status}" in metadata
    assert f"outer_status={status}" in metadata
    assert "R4 synthetic client result" in output
    assert "started=" in metadata and "finished=" in metadata


def test_biometric_ci_does_not_hide_a_log_writer_failure(tmp_path):
    done, metadata, output = run_biometric_command(tmp_path, 0, tee_status=73)
    assert done.returncode == 73, done.stdout + done.stderr
    assert "runner_status=0" in metadata and "tee_status=73" in metadata
    assert "outer_status=73" in metadata
    assert "R4 synthetic client result" in output


def test_biometric_ci_records_a_term_sent_to_its_own_wrapper(tmp_path):
    done, metadata, output = run_biometric_command(tmp_path, 0, signal=True)
    assert done.returncode == 143, done.stdout + done.stderr
    assert "outer_signal=TERM" in metadata and "outer_status=143" in metadata
    assert "R4 synthetic client result" in output
