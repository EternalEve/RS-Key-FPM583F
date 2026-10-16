#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Drive R4's real gate rows and CI commands over disposable fault controls."""

import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
LOGS = ROOT / "target/fpm-review-evidence"
BASELINE = "90580173960eb56e6e494b1771440a19c35ff98a"
sys.path.insert(0, str(ROOT / "scripts"))
import test_fingerprint_ci as wiring


def run(name, command, cwd, env=None):
    with (LOGS / f"r4-control-{name}.log").open("w") as log:
        done = subprocess.run(command, cwd=cwd, env=env, stdout=log, stderr=subprocess.STDOUT)
    output = (LOGS / f"r4-control-{name}.log").read_text()
    print(f"{name}: exit {done.returncode}", flush=True)
    return done.returncode, output


def require(code, output, expected, marker):
    assert code == expected, (code, expected, output[-4000:])
    assert marker in output, (marker, output[-4000:])


def main():
    LOGS.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "CARGO_NET_OFFLINE": "true", "CARGO_BUILD_JOBS": "2",
           "CARGO_TARGET_DIR": str(ROOT / "target/fpm-review-build")}
    source = (ROOT / "scripts/check.sh").read_text()
    config = re.search(r"^FINGERPRINT_BUILD_ENV=\([\s\S]*?^\)", source, re.M)[0]
    wrapper = re.search(r"^run\(\) \{[^\n]+", source, re.M)[0]
    with tempfile.TemporaryDirectory(prefix="rsk-r4-controls-") as temporary:
        work = Path(temporary)
        baseline = work / "baseline"
        for name in ("scripts/check.sh", "scripts/ci-scope.sh", ".github/workflows/emulator.yml"):
            target = baseline / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(subprocess.check_output(["git", "show", f"{BASELINE}:{name}"], cwd=ROOT))
        for name in ("test_fingerprint_ci.py", "gate_lines.py"):
            shutil.copy2(ROOT / "scripts" / name, baseline / "scripts" / name)
        code, output = run("before", [sys.executable, "-m", "pytest", "scripts", "-q",
                                     "-p", "no:cacheprovider", "--basetemp", str(work / "tests")], baseline)
        require(code, output, 1, "fingerprint candidate configuration is missing")
        assert "check.sh must run" in output and "emulator=false" in output
        assert "independent biometric CI job is missing" in output
        assert "ValueError" not in output and "ERROR collecting" not in output

        candidate = work / "candidate"
        candidate.mkdir()
        for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml"):
            shutil.copy2(ROOT / name, candidate / name)
        for name in (".cargo", "firmware", "crates", "rsk-wipe", "third_party/sequential-storage"):
            shutil.copytree(ROOT / name, candidate / name)

        def candidate_row(verb, settings=config):
            name = "clippy (fingerprint firmware)" if verb == "clippy" else "build firmware (fingerprint)"
            return ["bash", "-c", "\n".join(("set -euo pipefail", settings, wrapper, wiring.row(name)))]

        for verb in ("clippy", "build"):
            code, output = run(f"candidate-{verb}-clean", candidate_row(verb), candidate, env)
            require(code, output, 0, "Finished `release`")
            for module in ("fingerprint", "fingerprint_touch"):
                path = candidate / f"firmware/src/{module}.rs"
                original = path.read_text()
                marker = f"R4 injected {module} compiler failure"
                try:
                    path.write_text(original + f'\ncompile_error!("{marker}");\n')
                    code, output = run(f"{module}-{verb}", candidate_row(verb), candidate, env)
                    require(code, output, 101, marker)
                finally:
                    path.write_text(original)

        for pin, marker in ((2, "fingerprint power and touch overlap"),
                            (0, "fingerprint power overlaps UART0")):
            assert config.count("FINGERPRINT_POWER_PIN=3") == 1
            settings = config.replace("FINGERPRINT_POWER_PIN=3", f"FINGERPRINT_POWER_PIN={pin}")
            code, output = run(f"power-pin-{pin}", candidate_row("clippy", settings), candidate, env)
            require(code, output, 101, marker)
        code, output = run("candidate-restored", candidate_row("build"), candidate, env)
        require(code, output, 0, "Finished `release`")

        fault = work / "test_r4_client_failure.py"
        fault.write_text('# SPDX-License-Identifier: AGPL-3.0-only\n'
                         '# Copyright (C) 2026 RS-Key contributors\n'
                         'def test_r4_injected_client_failure():\n'
                         '    assert False, "R4 injected client assertion"\n')
        client_env = {**os.environ, "CARGO_NET_OFFLINE": "true", "CARGO_BUILD_JOBS": "2",
                      "PYTEST_ADDOPTS": f"{shlex.quote(str(fault))} -k test_r4_injected_client_failure"}
        for client in ("standard", "fingerprint"):
            code, output = run(f"{client}-assertion", ["nix", "develop", "-c",
                               f"./scripts/{client}-client.sh"], ROOT, client_env)
            require(code, output, 1, "R4 injected client assertion")
            assert "1 failed" in output and "deselected" in output

        log_dir = work / "bio-client-ci"
        command = wiring.biometric_command().replace("log_dir=target/bio-client-ci",
                                                      f"log_dir={shlex.quote(str(log_dir))}")
        code, output = run("biometric-assertion", ["bash", "-e", "-o", "pipefail", "-c", command],
                           ROOT, client_env)
        require(code, output, 1, "R4 injected client assertion")
        metadata = (log_dir / "status.txt").read_text()
        assert "runner_status=1" in metadata and "outer_status=1" in metadata
        shutil.copy2(log_dir / "status.txt", LOGS / "r4-control-biometric-assertion.status")
    print("R4 controls passed; expected compiler/assertion failures were refused", flush=True)


if __name__ == "__main__":
    main()
