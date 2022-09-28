set -e

cargo build -p canister_a --target wasm32-unknown-unknown --features export_api --release
cargo build -p canister_b --target wasm32-unknown-unknown --features export_api --release
cargo build -p canister_c --target wasm32-unknown-unknown --features export_api --release

ic-cdk-optimizer target/wasm32-unknown-unknown/release/canister_a.wasm -o target/wasm32-unknown-unknown/release/canister_a.wasm
ic-cdk-optimizer target/wasm32-unknown-unknown/release/canister_b.wasm -o target/wasm32-unknown-unknown/release/canister_b.wasm
ic-cdk-optimizer target/wasm32-unknown-unknown/release/canister_b.wasm -o target/wasm32-unknown-unknown/release/canister_b.wasm
