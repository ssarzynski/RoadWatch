#!/usr/bin/env bash
set -euo pipefail

echo "RoadWatch local verification"
echo "============================"

command -v cargo >/dev/null 2>&1 || {
  echo "ERROR: Rust/Cargo is required."
  exit 1
}

echo "[1/3] rustfmt"
cargo fmt --manifest-path verification-engine/Cargo.toml -- --check

echo "[2/3] clippy"
cargo clippy --manifest-path verification-engine/Cargo.toml --all-targets -- -D warnings

echo "[3/3] tests"
cargo test --manifest-path verification-engine/Cargo.toml

echo "PASS: RoadWatch verification-engine checks completed."
