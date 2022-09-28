set -e

cargo build -p canister-a --target wasm32-unknown-unknown --release
cargo build -p canister-b --target wasm32-unknown-unknown --release
cargo build -p canister-c --target wasm32-unknown-unknown --release

ic-cdk-optimizer target/wasm32-unknown-unknown/release/canister-a.wasm -o target/wasm32-unknown-unknown/release/canister-a-opt.wasm
ic-cdk-optimizer target/wasm32-unknown-unknown/release/canister-b.wasm -o target/wasm32-unknown-unknown/release/canister-b-opt.wasm
ic-cdk-optimizer target/wasm32-unknown-unknown/release/canister-c.wasm -o target/wasm32-unknown-unknown/release/canister-c-opt.wasm
