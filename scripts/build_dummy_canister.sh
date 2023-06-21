#!/bin/sh

cargo run -p dummy_canister --features export-api > ic-stable-structures/tests/dummy_canister/dummy_canister.did
cargo build -p dummy_canister --target wasm32-unknown-unknown --features export-api --release
ic-wasm target/wasm32-unknown-unknown/release/dummy_canister.wasm -o target/wasm32-unknown-unknown/release/dummy_canister.wasm shrink
gzip -k target/wasm32-unknown-unknown/release/dummy_canister.wasm --force
