#!/usr/bin/env sh
set -e
export RUST_BACKTRACE=full

# before testing, the build.sh script should be executed
cargo test 
cargo test --all-features