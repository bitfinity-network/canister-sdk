#[cfg(not(target_arch = "wasm32"))]
pub mod utils;

#[cfg(target_arch = "wasm32")]
pub mod state {
    mod canister_manager;

    pub use self::canister_manager::*;
}

#[cfg(target_arch = "wasm32")]
pub mod types {
    mod canister;
    mod checksum;

    pub use self::canister::*;
    pub use self::checksum::*;
}
