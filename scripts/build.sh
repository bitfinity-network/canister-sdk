set -e

cargo build -p canister-a --lib --target wasm32-unknown-unknown --features export-api --release
cargo build -p canister-b --lib --target wasm32-unknown-unknown --features export-api --release
cargo build -p canister-c --lib --target wasm32-unknown-unknown --features export-api --release
cargo build -p canister-d --lib --target wasm32-unknown-unknown --features export-api --release

ic-cdk-optimizer target/wasm32-unknown-unknown/release/canister_a.wasm -o target/wasm32-unknown-unknown/release/canister-a.wasm
ic-cdk-optimizer target/wasm32-unknown-unknown/release/canister_b.wasm -o target/wasm32-unknown-unknown/release/canister-b.wasm
ic-cdk-optimizer target/wasm32-unknown-unknown/release/canister_c.wasm -o target/wasm32-unknown-unknown/release/canister-c.wasm
ic-cdk-optimizer target/wasm32-unknown-unknown/release/canister_d.wasm -o target/wasm32-unknown-unknown/release/canister-d.wasm
