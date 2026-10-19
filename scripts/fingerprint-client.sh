#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors

set -euo pipefail
cd "$(dirname "$0")/.."

host_target="${HOST_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
cargo build --frozen --manifest-path tools/emu/Cargo.toml --target "$host_target" --features fingerprint-fixture,fido-ccid -j 2
work="$(mktemp -d /tmp/rsk-fingerprint-client.XXXXXX)"
trap 'rm -rf -- "$work"' EXIT
cp "tools/emu/target/$host_target/debug/rsk-emu" "$work/rsk-emu"
client_python="$(nix build --impure --file tests/standard_client/env.nix --no-link --print-out-paths)"

env -u PYTHONPATH -u PYTHONHOME \
  RSK_CLIENT_EMU="$work/rsk-emu" \
  "$client_python/bin/python" -m pytest tests/fingerprint_client \
  -q -s --tb=short -p no:cacheprovider --basetemp="$work/fingerprint-tests" "$@"
