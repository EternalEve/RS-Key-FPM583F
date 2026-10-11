#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors

set -euo pipefail
cd "$(dirname "$0")/.."

host_target="${HOST_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
cargo build --frozen --manifest-path tools/emu/Cargo.toml --target "$host_target" -j 2
client_python="$(nix build --impure --file tests/standard_client/env.nix --no-link --print-out-paths)"
work="$(mktemp -d /tmp/rsk-standard-client.XXXXXX)"
trap 'rm -rf -- "$work"' EXIT

# The dev shell's PYTHONPATH carries fido2 2.1.1; it must not shadow this isolated 2.2.1.
env -u PYTHONPATH -u PYTHONHOME \
  RSK_CLIENT_EMU="$PWD/tools/emu/target/$host_target/debug/rsk-emu" \
  "$client_python/bin/python" -m pytest tests/standard_client \
  -q -s --tb=short -p no:cacheprovider --basetemp="$work/tests" "$@"
