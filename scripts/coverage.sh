#!/usr/bin/env sh
set -e
export RUST_BACKTRACE=full

cargo +nightly tarpaulin --features memory-mapped-files-memory --verbose --timeout 120 --out xml --exclude canister-a --exclude canister-b --exclude canister-c --exclude canister-d --exclude canister-e --exclude test-payment-canister