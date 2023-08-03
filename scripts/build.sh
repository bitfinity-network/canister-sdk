set -e

cargo run -p canister-a --features export-api > ic-canister/tests/canister-a/canister-a.did
cargo run -p canister-b --features export-api > ic-canister/tests/canister-b/canister-b.did
cargo run -p canister-c --features export-api > ic-canister/tests/canister-c/canister-c.did
cargo run -p canister-d --features export-api > ic-canister/tests/canister-d/canister-d.did
cargo run -p dummy_canister --features export-api > ic-stable-structures/tests/dummy_canister/dummy_canister.did

cargo build -p canister-a --lib --target wasm32-unknown-unknown --features export-api --release
cargo build -p canister-b --lib --target wasm32-unknown-unknown --features export-api --release
cargo build -p canister-c --lib --target wasm32-unknown-unknown --features export-api --release
cargo build -p canister-d --lib --target wasm32-unknown-unknown --features export-api --release
cargo build -p dummy_canister --target wasm32-unknown-unknown --features export-api --release

ic-wasm target/wasm32-unknown-unknown/release/canister_a.wasm -o target/wasm32-unknown-unknown/release/canister-a.wasm shrink
ic-wasm target/wasm32-unknown-unknown/release/canister_b.wasm -o target/wasm32-unknown-unknown/release/canister-b.wasm shrink
ic-wasm target/wasm32-unknown-unknown/release/canister_c.wasm -o target/wasm32-unknown-unknown/release/canister-c.wasm shrink
ic-wasm target/wasm32-unknown-unknown/release/canister_d.wasm -o target/wasm32-unknown-unknown/release/canister-d.wasm shrink

