#!/usr/bin/env sh
set -e
export RUST_BACKTRACE=full

cargo tarpaulin --all-features --timeout 120 --out xml