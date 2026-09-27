#!/usr/bin/env bash
set -euo pipefail

echo "RoadWatch local verification"
echo "============================"

command -v cargo >/dev/null 2>&1 || {
  echo "ERROR: Rust/Cargo is required."
  exit 1
}

verify_crate() {
  local manifest="$1"
  local name="$2"
  echo
  echo "== $name =="
  cargo fmt --manifest-path "$manifest" -- --check
  cargo clippy --manifest-path "$manifest" --all-targets -- -D warnings
  cargo test --manifest-path "$manifest"
}

verify_crate verification-engine/Cargo.toml "verification engine"
verify_crate roadwatch-api/Cargo.toml "RoadWatch API"

echo
echo "PASS: RoadWatch local checks completed."
