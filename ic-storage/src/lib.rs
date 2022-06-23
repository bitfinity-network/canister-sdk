//! This crate provides a safe way to use canister state, as well as versioned storage.
//!
//! * For in memory storage use [`IcStorage`]. Structs that derive [`IcStorage`] must also implement `std::fmt::Default`.
//! * For versioned storage see [`crate::stable`].
//!
//! ```
//! # ic_canister::ic_kit::MockContext::new().inject();
//! use ic_storage::IcStorage;
//!
//! #[derive(IcStorage, Default)]
//! struct MyCanisterState {
//!     value: u32,
//! }
//!
//! let local_state = MyCanisterState::get();
//! assert_eq!(local_state.borrow().value, 0);
//! local_state.borrow_mut().value = 42;
//! assert_eq!(local_state.borrow().value, 42);
//! ```
//!
//! Unfortunately, you cannot use the derive macro to create storage for generic types, as there is
//! no way to know in advance, which concrete types are going to be stored. Instead, you can use
//! `generic_derive!` macro for them:
//!
//! ```
//! # ic_canister::ic_kit::MockContext::new().inject();
//! use ic_storage::IcStorage;
//!
//! #[derive(Default)]
//! struct GenericStorage<A, B> {
//!     val1: A,
//!     val2: B,
//! }
//!
//! ic_storage::generic_derive!(GenericStorage<u32, String>);
//!
//! let local_state = GenericStorage::<u32, String>::get();
//! assert_eq!(local_state.borrow().val1, 0);
//! assert_eq!(local_state.borrow().val2, "".to_string());
//! ```
//!
//! `IcStorage` derive macro uses `RefCell` to control the access to the state struct, so all the
//! borrow checks of `RefCell` apply to using `IcStorage` (e.g. trying to call `state.borrow_mut()`
//! when there is another borrow of the same type in scope will produce runtime panic).
//!
//! *IMPORTANT*: `IcStorage` only provides local canister state storage. It DOES NOT in any way
//! related to the stable storage. See [`crate::stable`] for stable storage.
//!
//! In order to preserve the canister data between canister upgrades you should
//! use either `ic_cdk::storage::stable_save()` and `ic_cdk::storage::stable_restore()`,
//! or [`crate::stable::read`] / [`crate::stable::write`].
//!
//! Any state that is not saved or restored using these methods will be lost if the canister
//! is upgraded.
//!
//! # Ways to use `IcStorage`
//!
//! There are two approaches for managing canister state:
//!
//! 1. You can have one big structure that contains all the state. In this case only this structure
//!    must implement `IcStorage`. This approach makes it easier to take care of storing the state
//!    in the stable storage on upgrade, and it's easier to manage the state when
//!    it's all in one place. On the other hand, it means that you can only have one mutable
//!    reference to this state during entire call, so you'll have to retrieve the state at the
//!    call entry point, and then call all inner functions with the state reference as argument.
//!    When there are many different parts of the state and they are not tightly connected to
//!    each other, it can become somewhat cumbersome.
//!
//! 2. You can have different types implementing `IcStorage`, thus making them completely independent.
//!    This approach is more flexible, especially for larger states, but special care must be taken
//!    not to forget any part of the state in the upgrade methods. Reading and writing to and from
//!    stable storage would require that everything that should be saved is passed as a single data
//!    type (e.g a tuple or a custom struct), as writing to stable storage overwrites what is
//!    currently there (meaning if we write one struct and then another, the second would overwrite
//!    the first).
//!
//! # Testing
//!
//! When running unit tests sometimes more than one state is needed to simulate different canister
//! instances. For that, `ic-storage` macros generate a little different code for architectures
//! other then `wasm32`. In this case you need to use `ic_canister::ic_kit` to set up the
//! `MockingContext` and set the current `id` in that context. For each `id` different storage will
//! be returned by `IcStorage::get()` method even in the same test case.

use std::cell::RefCell;
use std::rc::Rc;

pub use ic_storage_derive::IcStorage;

pub mod error;
pub mod stable;
pub use error::{Error, Result};
// #[cfg(test)]
pub mod testing;

/// Type that is stored in local canister state.
pub trait IcStorage {
    /// Returns the reference to the canister state. `RefCell` is used to prevent memory corruption
    /// for the state is the same object for all calls.
    fn get() -> Rc<RefCell<Self>>;
}

#[macro_export]
macro_rules! generic_derive {
    ($storage:ty) => {
        #[cfg(target_arch = "wasm32")]
        impl ::ic_cansiter::storage::IcStorage for $storage {
            fn get() -> ::std::rc::Rc<::std::cell::RefCell<Self>> {
                use ::std::rc::Rc;
                use ::std::cell::RefCell;

                thread_local! {
                    static store: Rc<RefCell<$storage>> = Rc::new(RefCell::new(<$storage>::default()));
                }

                store.with(|v| v.clone())
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        impl ::ic_canister::storage::IcStorage for $storage {
            fn get() -> ::std::rc::Rc<::std::cell::RefCell<Self>> {
                use ::std::rc::Rc;
                use ::std::cell::RefCell;
                use ::std::collections::HashMap;
                use ::candid::Principal;

                thread_local! {
                    static store: RefCell<HashMap<Principal, Rc<RefCell<$storage>>>> = RefCell::new(HashMap::default());
                }

                let id = ::ic_canister::ic_kit::ic::id();
                store.with(|v| {
                    let mut borrowed_store = v.borrow_mut();
                    (*borrowed_store.entry(id).or_default()).clone()
                })
            }
        }
    }
}
