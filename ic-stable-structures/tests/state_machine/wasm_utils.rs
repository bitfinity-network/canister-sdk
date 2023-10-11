use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use once_cell::sync::OnceCell;

/// Returns the bytecode of the evmc canister
pub fn get_dummy_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    CANISTER_BYTECODE
        .get_or_init(|| load_wasm_bytecode_or_panic("dummy_canister.wasm"))
        .to_owned()
}

fn load_wasm_bytecode_or_panic(wasm_name: &str) -> Vec<u8> {
    let path = get_path_to_wasm(wasm_name);

    let mut f = File::open(path).expect("File does not exists");

    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer)
        .expect("Could not read file content");

    buffer
}

fn get_path_to_wasm(wasm_name: &str) -> PathBuf {
    const ARTIFACT_PATH: &str = "../target/wasm32-unknown-unknown/release/";
    // Get to the root of the project
    let wasm_path = format!("{}{}", ARTIFACT_PATH, wasm_name);
    println!("path: {wasm_path:?}");
    if Path::new(&wasm_path).exists() {
        return wasm_path.into();
    } else {
        panic!("File {wasm_name} was not found in {ARTIFACT_PATH}");
    }
}
