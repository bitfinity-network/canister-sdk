//! Everyone who develops canisters for Internet Computer (IC) at some point faces same issues:
//!
//! * Testing canisters is hard. There's an `ic-kit` crate that allows you to abstract and test
//!   inner canister logic, but writing tests for inter-canister logic is still very difficult.
//!
//! * Coding inter-canister calls is tedious and error-prone.
//!
//! * It's usually impossible to have a cargo dependency of one canister for another canister.
//!   Because of that it's often necessary to duplicate types between canisters/test crates to
//!   facilitate inter-canister communications. Other solution is to have helper libraries for
//!   common types which increases complexity and adds restrictions on code organization.
//!
//! This crate's goal is to make writing and testing IC canisters easy and pleasant without
//! sacrificing safety and performance.
//!
//! # Canister structure
//!
//! To make a canister testable we need a standardized way to represent and mock the canisters. For
//! this the [Canister] trait with its derive macro is used. A structure implementing `Canister`
//! trait is the representation of the canister in IC. It contains the declaration of the
//! canister state and external API (`query` and `update` calls).
//!
//! A canister structure has follow these rules:
//!
//! * It must have exactly one `#[id]` field of type `Principal`. This is the canister id in the IC.
//!   This field is used to make inter-canister calls and mock the canister during testing.
//!
//! * It can have any number of `#[state]` fields of type `Rc<RefCell<T>>` where `T` must implement
//!   [ic_storage::IcStorage] trait. All the canister state must be declared here.
//!
//! * All the other fields (not marked with `#[id]` and `#[state]` must implement the `Default` trait.
//!
//! ```
//! use ic_cdk::export::Principal;
//! use ic_canister::Canister;
//! use ic_storage::IcStorage;
//! use std::cell::RefCell;
//! use std::rc::Rc;
//!
//! #[derive(Default, IcStorage)]
//! struct MyCanisterState {
//!     counter: u64,
//! }
//!
//! #[derive(Copy, Canister)]
//! struct MyCanister {
//!     #[id]
//!     principal: Principal,
//!
//!     #[state]
//!     state: Rc<RefCell<MyCanisterState>>,
//!
//!     other_field: u32,
//! }
//! ```
//!
//! The API of the canister can be declared using `#[query]` and `#[update]` macros. To prevent
//! incorrect invocation of API methods, the macros do not allow the API methods to be public. All
//! the arguments and output types must implement `CandidType` trait.
//!
//! ```
//! impl MyCanister {
//!     #[query]
//!     fn get_counter(&self) -> u64 {
//!         self.state.borrow().counter
//!     }
//!
//!     #[update]
//!     fn add(&self, value: u64) {
//!         self.state.borrow_mut().counter += value;
//!     }
//! }
//! ```
//!
//! When another canister needs to call these API methods, the `canister_call!()` macro can be used.
//!
//! ```
//! use ic_cdK::api::call::CallResult;
//!
//! let my_canister = MyCanister::from_principal(canister_principal);
//! canister_call(my_canister.add(10), ()).await.unwrap();
//! let counter: CallResult<u64> = canister_call!(my_canister.get_counter(), (u64)).await;
//! ```
//!
//! # Testing canisters
//!
//! ```
//! let my_canister = MyCanister::init_instance();
//! my_canister.add(1);
//! assert_eq!(my_canister.get_counter(), 1);
//! ```
//!
//! Using `Canister` approach even inter-canister logic can be tested seemlessly.

use ic_cdk::export::Principal;

pub use ic_canister_macros::*;

/// Main trait for a testable canister. Do not implement this trait manually, use the derive macro.
pub trait Canister {
    /// Creates a new instance of the canister with the default state. Call this method to initialize
    /// a canister for testing.
    ///
    /// In case of testing environment, this will create a canister with a random principal and
    /// store it in the LTS context.
    ///
    /// This method shall not be used directly in WASM environment (it is used internally by the
    /// API macros though).
    fn init_instance() -> Self;

    /// Initializes a new instance of the canister with the given principal. This method should be
    /// used by canisters that want to communicate with other canisters.
    ///
    /// In the testing environment, this method will return an instance previously initialized by
    /// the [Canister::init_instance] method. If the given principal was not initialized, or if the
    /// type of the canister is different from the type of invocation, the method will panic.
    fn from_principal(principal: Principal) -> Self;

    /// Returns the principal of the canister.
    fn principal(&self) -> Principal;
}
