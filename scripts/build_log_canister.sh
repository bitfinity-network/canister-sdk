#!/bin/sh

cargo run -p ic-log --example log_canister --features export-api > ic-log/examples/log_canister.did
cargo build -p ic-log --example log_canister --target wasm32-unknown-unknown --features export-api --release
ic-wasm target/wasm32-unknown-unknown/release/examples/log_canister.wasm -o target/wasm32-unknown-unknown/release/examples/log_canister.wasm shrink
gzip -k target/wasm32-unknown-unknown/release/examples/log_canister.wasm --force
