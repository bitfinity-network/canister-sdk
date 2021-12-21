<<<<<<< HEAD
mod errors;
pub mod utils;

pub use errors::{Error, Result};
=======
#[cfg(not(target_arch = "wasm32"))]
pub mod utils;

#[cfg(target_arch = "wasm32")]
pub mod factory {
    mod api;
    mod core;
    mod state;

    pub mod types {
        mod canister;
        mod checksum;

        pub use self::canister::*;
        pub use self::checksum::*;
    }

    pub use self::core::*;
    pub use self::state::*;
}
<<<<<<< HEAD
>>>>>>> 23876d9 (CPROD-300 add canister factory)
=======

#[cfg(target_arch = "wasm32")]
pub mod management {
    mod canister;
    pub use self::canister::*;
}

#[cfg(target_arch = "wasm32")]
pub mod is20 {
    mod principal_ext;
    pub use self::principal_ext::*;
}
>>>>>>> 41c43a7 (changes)
