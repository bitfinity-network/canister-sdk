#!/usr/bin/env sh
set -e
export RUST_BACKTRACE=full

cargo test 
cargo test --all-features