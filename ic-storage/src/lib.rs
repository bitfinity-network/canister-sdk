//! This crate provides a safe way to use canister state, as well as versioned storage. At the moment, ic_cdk storage has an
//! implementation bug, that make memory corruption possible <https://github.com/dfinity/cdk-rs/issues/73>.
//!
//! * For in memory storage use [`IcStorage`]. 
//!   Structs that derive [`IcStorage`] must also implement `std::fmt::Default`.
//! * For versioned storage see [`crate::stable`].
//!
//! ```
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

use std::cell::RefCell;
use std::rc::Rc;

pub use ic_storage_derive::IcStorage;

pub mod stable;
pub mod errors;
pub use errors::{Error, Result};

/// Type that is stored in local canister state.
pub trait IcStorage {
    /// Returns the reference to the canister state. `RefCell` is used to prevent memory corruption
    /// for the state is the same object for all calls.
    fn get() -> Rc<RefCell<Self>>;
}

#[macro_export]
macro_rules! generic_derive {
    ($storage:ty) => {
        impl ::ic_storage::IcStorage for $storage {
            fn get() -> ::std::rc::Rc<::std::cell::RefCell<Self>> {
                use ::std::rc::Rc;
                use ::std::cell::RefCell;

                thread_local! {
                    static store: Rc<RefCell<$storage>> = Rc::new(RefCell::new(<$storage>::default()));
                }

                store.with(|v| v.clone())
            }
        }
    }
}
