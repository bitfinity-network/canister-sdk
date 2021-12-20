<<<<<<< HEAD
mod errors;
pub mod utils;

pub use errors::{Error, Result};
=======
#[cfg(not(target_arch = "wasm32"))]
pub mod utils;

#[cfg(target_arch = "wasm32")]
pub mod factory {
    mod canisters_factory;

    pub mod types {
        mod canister;
        mod checksum;

        pub use self::canister::*;
        pub use self::checksum::*;
    }

    pub use self::canisters_factory::*;
}
>>>>>>> 23876d9 (CPROD-300 add canister factory)
