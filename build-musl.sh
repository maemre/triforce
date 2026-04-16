#!/usr/bin/env bash
set -euo pipefail

TARGET=x86_64-unknown-linux-musl

cargo build --release --target "$TARGET" --bin check-given-covers --no-default-features

echo "Binary: target/$TARGET/release/check-given-covers"
