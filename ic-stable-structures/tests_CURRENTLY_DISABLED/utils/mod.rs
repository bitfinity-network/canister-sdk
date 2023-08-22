use std::path::{Path, PathBuf};

pub mod wasm;

/// Returns the Path to the workspace root dir
pub fn get_workspace_root_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
