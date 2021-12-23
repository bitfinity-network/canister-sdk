//! This crate provides a safe way to use canister state. At the moment, ic_cdk storage has an
//! implementation bug, that make memory corruption possible (https://github.com/dfinity/cdk-rs/issues/73).
//!
//! To use storage with this crate, use [IcStorage] derive macro. Structs, that use it must also
//! implement `Default` trait.
//!
//! ```
//! use ic_storage::IcStorage;
//! use std::rc::Rc;
//! use std::cell::RefCell;
//!
//! #[derive(IcStorage, Default)]
//! struct MyCanisterState {
//!     value: u32,
//! }
//!
//! let local_state: Rc<RefCell<MyCanisterState>> = MyCanisterState::get();
//! println!("Current value: {}", local_state.borrow().value);
//! RefCell::borrow_mut(&*local_state).value = 42;
//! ```
//!
//! *IMPORTANT*: `IcStorage` only provides local canister state storage. It DOES NOT in any way
//! related to the stable storage. In order to preserve the canister data between canister
//! upgrades you must use `ic_cdk::storage::stable_save()` and `ic_cdk::storage::stable_restore()`.
//! Any state that is not saved and restored with these methods will be lost when the canister
//! is upgraded. On the details how to do it, check out the Rust coding conventions page in
//! confluence.
//!
//! # Ways to use `IcStorage`
//!
//! There are two approaches for managing canister state:
//!
//! 1. You can have one big structure that contains all the state. In this case only this structure
//!    must implement `IcStorage`. This approach makes it easier to take care for storing the state
//!    in the stable storage on upgrade, and just in general it's easier to manage the state when
//!    it's all in one place. On the other hand, it means that you can have only one mutable
//!    reference to this state during entire call, so you'll have to retrieve the state at the
//!    call entry point, and then call all inner functions with the state reference as argument.
//!    When there are many different parts of the state and they are not tightly connected to
//!    each other, it can become somewhat cumbersome.
//!
//! 2. You can have different types implementing `IcStorage`, thus making them completely independent.
//!    This approach is more flexible, especially for larger states, but special care must be taken
//!    not to forget any part of the state in the upgrade methods.

use std::rc::Rc;
use std::cell::RefCell;

pub use ic_storage_derive::IcStorage;

/// Type that is stored in local canister state.
pub trait IcStorage {
    /// Returns the reference to the canister state. `RefCell` is used to prevent memory corruption
    /// for the state is the same object for all calls.
    fn get() -> Rc<RefCell<Self>>;
}