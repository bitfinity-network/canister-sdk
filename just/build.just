
# Cleans the build artifacts
[group('build')]
[confirm("Are you sure you want to clean the build artifacts?")]
clean:
  rm -rf {{WASM_DIR}}
  cargo clean


# Builds all artifacts
[group('build')]
build: pre_build build_ic_stable_structures_dummy_canister build_ic_canister_test_canisters build_ic_task_scheduler_dummy_scheduler_canister build_ic_log_test_canister build_ic_payments_test_canister build_http_proxy_canister


[private]
pre_build: 
  mkdir -p "{{WASM_DIR}}"
  

[private]
build_ic_stable_structures_dummy_canister:
  cargo run -p dummy_canister --features export-api > {{WASM_DIR}}/dummy_canister.did
  cargo build -p dummy_canister --target wasm32-unknown-unknown --features export-api --release
  ic-wasm {{WASM_DIR}}/dummy_canister.wasm -o {{WASM_DIR}}/dummy_canister.wasm shrink


[private]
build_ic_canister_test_canisters:
  cargo run -p canister-a --features export-api > {{WASM_DIR}}/canister-a.did
  cargo run -p canister-b --features export-api > {{WASM_DIR}}/canister-b.did
  cargo run -p canister-c --features export-api > {{WASM_DIR}}/canister-c.did
  cargo run -p canister-d --features export-api > {{WASM_DIR}}/canister-d.did

  cargo build -p canister-a --target wasm32-unknown-unknown --features export-api --release
  cargo build -p canister-b --target wasm32-unknown-unknown --features export-api --release
  cargo build -p canister-c --target wasm32-unknown-unknown --features export-api --release
  cargo build -p canister-d --target wasm32-unknown-unknown --features export-api --release

  ic-wasm {{WASM_DIR}}/canister-a.wasm -o {{WASM_DIR}}/canister-a.wasm shrink
  ic-wasm {{WASM_DIR}}/canister-b.wasm -o {{WASM_DIR}}/canister-b.wasm shrink
  ic-wasm {{WASM_DIR}}/canister-c.wasm -o {{WASM_DIR}}/canister-c.wasm shrink
  ic-wasm {{WASM_DIR}}/canister-d.wasm -o {{WASM_DIR}}/canister-d.wasm shrink


[private]
build_ic_task_scheduler_dummy_scheduler_canister:
  cargo run -p dummy_scheduler_canister --features export-api > {{WASM_DIR}}/dummy_scheduler_canister.did
  cargo build -p dummy_scheduler_canister --target wasm32-unknown-unknown --features export-api --release
  ic-wasm {{WASM_DIR}}/dummy_scheduler_canister.wasm -o {{WASM_DIR}}/dummy_scheduler_canister.wasm shrink


[private]
build_ic_log_test_canister:
  cargo run -p ic-log --example log_canister --features canister > {{WASM_DIR}}/log_canister.did
  cargo build -p ic-log --example log_canister --target wasm32-unknown-unknown --features canister --release
  ic-wasm {{WASM_DIR}}/examples/log_canister.wasm -o {{WASM_DIR}}/log_canister.wasm shrink


[private]
build_ic_payments_test_canister:
  #!/usr/bin/env bash

  # Get example icrc1 canister
  if [ ! -f {{WASM_DIR}}/ic-icrc1-ledger.wasm ]; then
      export IC_VERSION=4824fd13586f1be43ea842241f22ee98f98230d0
      echo curl
      curl -o {{WASM_DIR}}/ic-icrc1-ledger.wasm.gz https://download.dfinity.systems/ic/${IC_VERSION}/canisters/ic-icrc1-ledger.wasm.gz
      echo gun
      gunzip {{WASM_DIR}}/ic-icrc1-ledger.wasm.gz
  fi

  cargo build --target wasm32-unknown-unknown --features export-api -p test-payment-canister --release
  ic-wasm {{WASM_DIR}}/test-payment-canister.wasm -o {{WASM_DIR}}/test-payment-canister.wasm shrink

[private]
build_http_proxy_canister:
  cargo run -p ic-http-outcall-proxy-canister --features export-api > {{WASM_DIR}}/ic-http-outcall-proxy-canister.did
  cargo build -p ic-http-outcall-proxy-canister --target wasm32-unknown-unknown --features export-api --release
  ic-wasm {{WASM_DIR}}/ic-http-outcall-proxy-canister.wasm -o {{WASM_DIR}}/ic-http-outcall-proxy-canister.wasm shrink

