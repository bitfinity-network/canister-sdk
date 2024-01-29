#!/usr/bin/env sh
set -e
set -x #echo on

export RUST_BACKTRACE=full

WASM_DIR="target/wasm32-unknown-unknown/release"

build_ic_canister_test_canisters() {
    echo "Building ic-canister test canisters"

    cargo run -p canister-a --features export-api > $WASM_DIR/canister-a.did
    cargo run -p canister-b --features export-api > $WASM_DIR/canister-b.did
    cargo run -p canister-c --features export-api > $WASM_DIR/canister-c.did
    cargo run -p canister-d --features export-api > $WASM_DIR/canister-d.did

    cargo build -p canister-a --target wasm32-unknown-unknown --features export-api --release
    cargo build -p canister-b --target wasm32-unknown-unknown --features export-api --release
    cargo build -p canister-c --target wasm32-unknown-unknown --features export-api --release
    cargo build -p canister-d --target wasm32-unknown-unknown --features export-api --release

    ic-wasm $WASM_DIR/canister-a.wasm -o $WASM_DIR/canister-a.wasm shrink
    ic-wasm $WASM_DIR/canister-b.wasm -o $WASM_DIR/canister-b.wasm shrink
    ic-wasm $WASM_DIR/canister-c.wasm -o $WASM_DIR/canister-c.wasm shrink
    ic-wasm $WASM_DIR/canister-d.wasm -o $WASM_DIR/canister-d.wasm shrink
}

build_ic_stable_structures_dummy_canister() {
    echo "Building ic-stable-structures dummy canister"

    cargo run -p dummy_canister --features export-api > $WASM_DIR/dummy_canister.did
    cargo build -p dummy_canister --target wasm32-unknown-unknown --features export-api --release
    ic-wasm $WASM_DIR/dummy_canister.wasm -o $WASM_DIR/dummy_canister.wasm shrink

}

build_ic_task_scheduler_dummy_scheduler_canister() {
    echo "Building ic-task-scheduler dummy_scheduler_canister"

    cargo run -p dummy_scheduler_canister --features export-api > $WASM_DIR/dummy_scheduler_canister.did
    cargo build -p dummy_scheduler_canister --target wasm32-unknown-unknown --features export-api --release
    ic-wasm $WASM_DIR/dummy_scheduler_canister.wasm -o $WASM_DIR/dummy_scheduler_canister.wasm shrink

}

build_ic_log_test_canister() {
    echo "Building ic-log test canister"

    cargo run -p ic-log --example log_canister --features export-api > $WASM_DIR/log_canister.did
    cargo build -p ic-log --example log_canister --target wasm32-unknown-unknown --features export-api --release
    ic-wasm $WASM_DIR/examples/log_canister.wasm -o $WASM_DIR/log_canister.wasm shrink

}

build_ic_payments_test_canister() {
    echo "Building ic-payments test canister"

    # Get example icrc1 canister
    if [ ! -f $WASM_DIR/ic-icrc1-ledger.wasm ]; then
        export IC_VERSION=4824fd13586f1be43ea842241f22ee98f98230d0
        echo curl
        curl -o $WASM_DIR/ic-icrc1-ledger.wasm.gz https://download.dfinity.systems/ic/${IC_VERSION}/canisters/ic-icrc1-ledger.wasm.gz
        echo gun
        gunzip $WASM_DIR/ic-icrc1-ledger.wasm.gz
    fi

    echo build
    cargo build --target wasm32-unknown-unknown --features export-api -p test-payment-canister --release
    echo wasm
    ic-wasm $WASM_DIR/test-payment-canister.wasm -o $WASM_DIR/test-payment-canister.wasm shrink
}

main() {
    mkdir -p $WASM_DIR

    build_ic_canister_test_canisters
    build_ic_stable_structures_dummy_canister
    build_ic_task_scheduler_dummy_scheduler_canister
    build_ic_log_test_canister
    build_ic_payments_test_canister

}

main "$@"