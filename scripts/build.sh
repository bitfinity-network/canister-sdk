set -e

cargo build -p canister-a --target wasm32-unknown-unknown --features export-api --release
cargo build -p canister-b --target wasm32-unknown-unknown --features export-api --release
cargo build -p canister-c --target wasm32-unknown-unknown --features export-api --release

ic-cdk-optimizer target/wasm32-unknown-unknown/release/canister-a.wasm -o target/wasm32-unknown-unknown/release/canister-a.wasm
ic-cdk-optimizer target/wasm32-unknown-unknown/release/canister-b.wasm -o target/wasm32-unknown-unknown/release/canister-b.wasm
ic-cdk-optimizer target/wasm32-unknown-unknown/release/canister-c.wasm -o target/wasm32-unknown-unknown/release/canister-c.wasm
