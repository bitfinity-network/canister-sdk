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
//!   Note, that when the canister is deployed in IC, the `Canister` instance is transient. It means
//!   that all non-state fields will have default values at the beginning of each call.
//!
//! ```ignored
//! use std::cell::RefCell;
//! use std::rc::Rc;
//! use ic_exports::ic_cdk::export::candid::{Principal, CandidType, Deserialize};
//! use ic_storage::stable::Versioned;
//! use ic_canister::{Canister, PreUpdate};
//! use ic_canister::storage::IcStorage;
//!
//! #[derive(Default, IcStorage, CandidType, Deserialize)]
//! struct MyCanisterState {
//!     counter: u64,
//! }
//!
//! impl Versioned for MyCanisterState {
//!     type Previous = ();
//!
//!     fn upgrade(():()) -> Self {
//!         Self { counter: 0 }
//!     }
//! }
//!
//! #[derive(Clone, Canister)]
//! struct MyCanister {
//!     #[id]
//!     principal: Principal,
//!
//!     #[state]
//!     state: Rc<RefCell<MyCanisterState>>,
//!
//!     other_field: u32,
//! }
//!
//! impl PreUpdate for MyCanister {}
//! ```
//!
//! # Canister lifetime
//!
//! ## Initialization
//!
//! The canister initialization method can be declared with [init] macro.
//!
//! ```
//! use ic_exports::ic_cdk::export::Principal;
//! use ic_canister::{Canister, PreUpdate, init};
//!
//! #[derive(Clone, Canister)]
//! struct MyCanister {
//!     #[id]
//!     principal: Principal,
//! }
//!
//! impl MyCanister {
//!     #[init]
//!     fn init(&self, init_param: String) {
//!         // initialization code here
//!     }
//! }
//!
//! impl PreUpdate for MyCanister {}
//! ```
//!
//! `#[init]` method must follow the following rules:
//! * there must be only one `#[init]` method in a canister
//! * it must have no return type
//! * it must be an instance method, taking `self` by reference
//!
//! ## Upgrading
//!
//! If there is only one `#[state]` field, `Canister` derive macro will generate `pre_upgrade` and
//! `post_upgrade` methods automatically. These methods will serialize the state to the stable
//! storage on `pre_upgrade` and then use `canister_sdk::ic_storage::stable::Versioned` trait to upgrade the state
//! in `post_upgrade`. This approach has some limitations:
//!
//! * The canister can contain only one `#[state]` field. If more then one state fields are needed,
//!   but only one must be stored in the stable memory, the other fields can me marked with
//!   `#[state(stable_store = false)]`.
//! * The state structure must implement the `Versioned`, `CandidType` and `Deserialize` traits.
//! * No other data can be stored in the stable storage.
//!
//! If any of these conditions is not true, upgrade methods generation can be skipped by adding
//! `#[canister_no_upgrade_methods]` attribute to the canister structure. In this case use
//! `#[ic_canister::pre_upgrade]` and `#[ic_canister::post_upgrade]` macros to mark the corresponding
//! manual implementations if needed.
//!
//! # API
//!
//! The API of the canister can be declared using `#[query]` and `#[update]` macros. To prevent
//! incorrect invocation of API methods, the macros do not allow the API methods to be public. All
//! the arguments and output types must implement `CandidType` trait.
//!
//! ```ignore
//! use candid::{Principal, CandidType, Deserialize};
//! use ic_canister::{Canister, PreUpdate, query, update};
//! use ic_canister::storage::IcStorage;
//! use std::cell::RefCell;
//! use std::rc::Rc;
//!
//! #[derive(Default, IcStorage, CandidType, Deserialize)]
//! struct MyCanisterState {
//!     counter: u64,
//! }
//! # impl Versioned for MyCanisterState {
//! #     type Previous = ();
//! #     fn upgrade(():()) -> Self {
//! #         Self { counter: 0 }
//! #     }
//! # }
//!
//! #[derive(Clone, Canister)]
//! struct MyCanister {
//!     #[id]
//!     principal: Principal,
//!
//!     #[state]
//!     state: Rc<RefCell<MyCanisterState>>,
//! }
//!
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
//!
//! impl PreUpdate for MyCanister {}
//! ```
//!
//! The API methods must be instance methods (taking `self` by reference).
//!
//! # Traits as canisters
//!
//! When we want to enrich a canister with some generic structure, we can define a trait that the
//! given canister will implement, for instance:
//!
//! ```ignore
//! struct FactoryState;
//!
//! trait FactoryAPI {
//!     fn factory_state(&self) -> Rc<RefCell<FactoryState>> {
//!         FactoryState::get()
//!     }
//!
//!     fn get_all(&self) -> Vec<Canister> {
//!       self.factory_state().canisters.get_all()
//!     }
//! }
//! ```
//!
//! There are a few limitations that we're currently facing with this approach, in particular:
//!
//! ### For each trait an exporting struct must be defined.
//!
//! There's a limitation with trait methods in rust that unfortunately we cannot
//! give attributes to them. This is because monomorphisation in rust is optional
//! to allow for trait objects to exist, hence it's quite a non-trivial task to
//! export these implementation-dependent functions to wasm. What we are currently
//! forcing the user to do is to define an exporting struct, which will be used as
//! a buffer, that we'll generate methods for and export to wasm via [generate_exports]
//! macro. This is done on the previous example for `TokenFactory`.
//!
//! ### Each trait canister must have only one function marked as `#[state_getter]`.
//!
//! There's a pecularity with default implementation of the state getter via `State::get()` call.
//! Under the hood it creates a lazy static storage that is only crate-local, meaning that
//! the default implementation of the trait canister will work with its own storage, instead of
//! the struct that implements this trait. Hence, the user has to overwrite this getter with its
//! own `State::get()` call. The `#[state_getter]` macro is used to mark the function as the
//! state getter that will be later used in [`generate_exports!`] macro.
//!
//! ### Each trait method must be marked with `#[update/query(trait = true)]` macro.
//!
//! This is to allow for [generate_exports!] to actually collect all of the methods to
//! be able to export them further to wasm.
//!
//! ### Each async function in trait definition must return a pinned future.
//!
//! Since async functions in traits are [hard](https://smallcultfollowing.com/babysteps/blog/2019/10/26/async-fn-in-traits-are-hard/), and due to the order of macro expansion
//! we cannot use `#[async_trait]` macro for our trait. Instead we need to return a
//! `Pin<Box<dyn Future<Output = T>>>` where `T` is the output type of the async function.
//! There's a type alias [AsyncReturn] defined for it.
//!
//! ### Lifetime specifiers for async methods in non-trait canister needs to be defined.
//!
//! This is an implementation detail because rust cannot infer lifetime for
//! `Pin<Box<impl Future<Output = T>> + 'self>>` when needed.
//!
//! ### `get_idl()` method needs to be implemented for a trait canister to allow for exporting idl to other crates.
//!
//! This is an implementation detail. Since macros are expanded at compile time, the
//! [generate_idl] macro will always be constant *and* crate-local. This is because under
//! the hood this macro uses local `lazy_static` variables to allow for sharing state
//! between macro expansions. Unfortunately we cannot export this static state explicitly
//! in procedural crates such as [ic-canister-macros] (a rust limitation that is being discussed since 2016).
//! Also, macroses themself cannot export concrete values (unless primitives that implement `ToTokens` trait).
//! But there's a workaround. We, can make a method `get_idl()` for trait that we want
//! to export idl from and generate a code via macro, that will return more sensible
//! struct ([Idl] in this case) that later can be exported by the crate. This is what
//! we're essentially doing in the `ic-factory::FactoryCanister` trait.
//!
//! Note: Since [generate_idl] macro depends on the order of the method macro expansion,
//! it's adviced to put the declaration of `get_idl()` method to the end of the trait
//! definition.
//!
//! And since this dependency will be compiled from other crate's perspective, the code
//! for `get_idl()` will be constant and will return the necessary struct which will be
//! merged via [Idl::merge] with idl of the canister, we're implementing, like
//!
//! ```ignore
//! let canister_idl = ic_canister::generate_idl!();
//! let mut factory_idl = <TokenFactoryCanister as FactoryCanister>::get_idl();
//! factory_idl.merge(&canister_idl);
//!
//! let result = candid::bindings::candid::compile(&factory_idl.env.env, &Some(factory_idl.actor));
//! println!("{result}");
//! ```
//!
//! # Inter-canister calls
//!
//! When another canister needs to call these API methods, the [canister_call]` macro can be used.
//!
//! ```ignore
//! use ic_exports::ic_cdk::api::call::CallResult;
//!
//! let my_canister = MyCanister::from_principal(canister_principal);
//! canister_call!(my_canister.add(10), ()).await.unwrap();
//! let counter: CallResult<u64> = canister_call!(my_canister.get_counter(), (u64)).await;
//! ```
//!
//! ## Virtual canister calls
//!
//! Often you want to make a remote call to a canister that was not written using `ic-canister` crate.
//! In this case you don't have a [Canister] trait implementation, so the `canister_call` macro
//! cannot be used. Instead, you can use [virtual_canister_call] macro.
//!
//! ```ignore
//! use ic_exports::ic_cdk::api::call::CallResult;
//! use ic_exports::ic_cdk::export::Principal;
//! use ic_canister::virtual_canister_call;
//!
//! let principal = Principal::from_text("qd4yy-7yaaa-aaaag-aacdq-cai").unwrap();
//! let result: CallResult<ReturnType> = virtual_canister_call!(principal, "remote_method_name", (arg1, arg2), ReturnType).await;
//! ```
//!
//! //! # Inter-canister notifications
//!
//! When another canister needs to call these API methods with one-way messages, the [canister_notify]` macro can be used.
//!
//! ```ignore
//! use ic_exports::ic_cdk::api::call::RejectionCode;
//!
//! let my_canister = MyCanister::from_principal(canister_principal);
//! let result: Result<(), ic_exports::ic_cdk::api::call::RejectionCode> = canister_notify!(my_canister.add(10), ());
//! ```
//!
//! ## Virtual canister notifications
//!
//! Often you want to make a one-way remote call to a canister that was not written using `ic-canister` crate.
//! In this case you don't have a [Canister] trait implementation, so the `canister_notify` macro
//! cannot be used. Instead, you can use [virtual_canister_notify] macro.
//!
//! ```ignore
//! use ic_exports::ic_cdk::export::Principal;
//! use ic_canister::virtual_canister_notify;
//!
//! let principal = Principal::from_text("qd4yy-7yaaa-aaaag-aacdq-cai").unwrap();
//! let result: Result<(), ic_exports::ic_cdk::api::call::RejectionCode>= virtual_canister_notify!(principal, "remote_method_name", (arg1, arg2), ());
//! ```
//!
//! # Testing canisters
//!
//! ## Internal canister logic
//!
//! A canister instance for testing can be created using [Canister::init_instance()] method.
//!
//! The states of every created instance will be separate.
//!
//! ```ignore
//! # use candid::{Principal, CandidType, Deserialize};
//! # use ic_canister::{Canister, PreUpdate, query, update};
//! # use ic_canister::storage::IcStorage;
//! # use std::cell::RefCell;
//! # use std::rc::Rc;
//! #
//! # ic_exports::ic_kit::MockContext::new().inject();
//! #
//! # #[derive(Default, IcStorage, CandidType, Deserialize)]
//! # struct MyCanisterState {
//! #     counter: u64,
//! # }
//! #
//! # impl Versioned for MyCanisterState {
//! #     type Previous = ();
//! #     fn upgrade(():()) -> Self {
//! #         Self { counter: 0 }
//! #     }
//! # }
//! #
//! # #[derive(Clone, Canister)]
//! # struct MyCanister {
//! #     #[id]
//! #     principal: Principal,
//! #
//! #     #[state]
//! #     state: Rc<RefCell<MyCanisterState>>,
//! # }
//! #
//! # impl MyCanister {
//! #     #[query]
//! #     fn get_counter(&self) -> u64 {
//! #         self.state.borrow().counter
//! #     }
//! #
//! #     #[update]
//! #     fn add(&self, value: u64) {
//! #         self.state.borrow_mut().counter += value;
//! #     }
//! # }
//! #
//! # impl PreUpdate for MyCanister {}
//!
//! let my_canister = MyCanister::init_instance();
//! my_canister.add(1);
//! assert_eq!(my_canister.get_counter(), 1);
//! my_canister.add(1);
//! assert_eq!(my_canister.get_counter(), 2);
//!
//! let another_instance = MyCanister::init_instance();
//! another_instance.add(1);
//! assert_eq!(another_instance.get_counter(), 1);
//! assert_eq!(my_canister.get_counter(), 2);
//! ```
//!
//! You can retrieve a previously created canister using it's principal id and the
//! [Canister::from_principal] method. In this case a new instance of a canister is created, but
//! it shares the state with the first instance.
//!
//! ```ignore
//! # use candid::{Principal, CandidType, Deserialize};
//! # use ic_canister::{Canister, PreUpdate, query, update};
//! # use ic_canister::storage::IcStorage;
//! # use std::cell::RefCell;
//! # use std::rc::Rc;
//! #
//! # ic_exports::ic_kit::MockContext::new().inject();
//! #
//! # #[derive(Default, IcStorage, CandidType, Deserialize)]
//! # struct MyCanisterState {
//! #     counter: u64,
//! # }
//! #
//! # impl Versioned for MyCanisterState {
//! #     type Previous = ();
//! #     fn upgrade(():()) -> Self {
//! #         Self { counter: 0 }
//! #     }
//! # }
//! #
//! # #[derive(Clone, Canister)]
//! # struct MyCanister {
//! #     #[id]
//! #     principal: Principal,
//!
//! #     #[state]
//! #     state: Rc<RefCell<MyCanisterState>>,
//! # }
//! #
//! # impl MyCanister {
//! #     #[query]
//! #     fn get_counter(&self) -> u64 {
//! #         self.state.borrow().counter
//! #     }
//! #
//! #     #[update]
//! #     fn add(&self, value: u64) {
//! #         self.state.borrow_mut().counter += value;
//! #     }
//! # }
//! #
//! # impl PreUpdate for MyCanister {}
//!
//! let my_canister = MyCanister::init_instance();
//! my_canister.add(1);
//! assert_eq!(my_canister.get_counter(), 1);
//!
//! let principal = my_canister.principal();
//!
//! let another_instance = MyCanister::from_principal(principal);
//! another_instance.add(1);
//! assert_eq!(another_instance.get_counter(), 2);
//! assert_eq!(my_canister.get_counter(), 2);
//! ```
//!
//! ## Testing inter-canister calls
//!
//! If you want to test a canister method that internally calls another canister using [canister_call]
//! macro the only thing you need to do is initialize the instance of the second canister before
//! the second canister will call it.
//!
//! ```ignore
//! use ic_exports::ic_cdk::api::call::CallResult;
//! use ic_exports::ic_cdk::export::Principal;
//! use ic_canister::{Canister, update, canister_call};
//!
//! impl SecondCanister {
//!     #[update]
//!     async fn make_remote_call(&self, principal: Principal) -> CallResult<()>{
//!         let canister = FirstCanister::from_principal(principal);
//!         canister_call!(canister.remote_method()).await
//!     }
//! }
//! let first_canister = FirstCanister::init_instance();
//! let second_canister = SecondCanister::init_instance();
//!
//! second_canister.make_remote_call(first_canister.principal())
//! ```
//!
//! If the remote call is made using [virtual_canister_call] macro, a function must be registered
//! that will respond to such a call.
//!
//! ```ignore
//! use ic_exports::ic_cdk::api::call::CallResult;
//! use ic_exports::ic_cdk::export::Principal;
//! use ic_canister::{Canister, register_virtual_responder, update, virtual_canister_call};
//!
//! impl SecondCanister {
//!     #[update]
//!     async fn make_remote_call(&self, principal: Principal, value: u32) -> CallResult<u64>{
//!         virtual_canister_call!(principal, "remote_method", (value,), u64).await
//!     }
//! }
//!
//! let second_canister = SecondCanister::init_instance();
//! let principal = Principal::from_text("qd4yy-7yaaa-aaaag-aacdq-cai").unwrap();
//!
//! register_virtual_responder(principal, "remote_method", |(arg,): (u32,)| arg as u64);
//!
//! assert_eq!(second_canister.make_remote_call(principal, 10).unwrap(), 10u64);
//! ```
//!
//! If you want to test a virtual call in case the call fails, [register_failing_virtual_responder]
//! function can be used.
//!
//! # Canister crates dependencies
//!
//! By default the canister declaration will export its API when compiled for `wasm32-unknown-unknown`
//! target. This means, that if a canister depends on another canister, both canister methods will
//! be exported and can be called by IC API.
//!
//! Sometimes this behaviour is not desired. For example, we want a canister to make an
//! inter-canister call to another canister, so we add the second one as a dependence to the first one
//! to use its types and method declarations, and to be able to test their logic together, but
//! we don't want the second canister's API to be exported together with the first one's.
//!
//! To enable this, a canister crate can declare a `export_api` feature flag. If this flag is disabled,
//! no API methods of the dependency canister will be exported.
//!
//! ```ignore
//! // child canister
//! [features]
//! default = []
//! export_api = []
//!
//! // parent canister
//! [dependency]
//! child_canister = {version = "1.0", features = ["export_api"]}
//! ```
//!
//! Note though, that the Cargo features are additive during same compilation process. It means, that
//! if you try to compile both `child_canister` and `parent_canister` with the same `cargo` invocation,
//! the `child_canister` will be compiled without API. So if you have several canisters in the same
//! cargo workspace with dependencies between them, you will have to compile and test them separately.
//!
//! # Generating idl
//!
//! You can generate IDL (Candid) definition for your canister using [generate_idl] macro and then compile it via `candid::bindings::candid::compile()`.

use ic_exports::ic_cdk::{
    api::call::{CallResult, RejectionCode},
    export::{
        candid::{self, utils::ArgumentDecoder, CandidType, Deserialize},
        Principal,
    },
};
use std::{cell::RefCell, collections::HashMap, future::Future, pin::Pin, rc::Rc};

pub use ic_canister_macros::*;

pub mod idl;
pub use idl::*;

pub enum MethodType {
    Query,
    Update,
    Oneway,
}

impl<T: AsRef<str>> From<T> for MethodType {
    fn from(s: T) -> Self {
        match s.as_ref() {
            "query" => MethodType::Query,
            "update" => MethodType::Update,
            "oneway" => MethodType::Oneway,
            _ => panic!("Unknown method type: {}", s.as_ref()),
        }
    }
}

impl std::fmt::Display for MethodType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MethodType::Query => write!(f, "query"),
            MethodType::Update => write!(f, "update"),
            MethodType::Oneway => write!(f, "oneway"),
        }
    }
}

pub trait PreUpdate {
    fn pre_update(&self, _method_name: &str, _method_type: MethodType) {}
}

/// Main trait for a testable canister. Do not implement this trait manually, use the derive macro.
pub trait Canister: PreUpdate {
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

// Important: If you're renaming this type, don't forget to update
// the `ic_canister_macros::api::get_args` as well.
pub type AsyncReturn<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

type ResponderFn = dyn Fn(Vec<u8>) -> CallResult<Vec<u8>>;
type ResponderHashMap = HashMap<(Principal, String), Box<ResponderFn>>;

thread_local! {
    static __RESPONDERS: Rc<RefCell<ResponderHashMap>> = Rc::new(RefCell::new(HashMap::new()));
}

fn _register_virtual_responder(
    principal: Principal,
    method_name: &str,
    responder: impl Fn(Vec<u8>) -> CallResult<Vec<u8>> + 'static,
) {
    __RESPONDERS.with(|responders| {
        responders
            .borrow_mut()
            .insert((principal, method_name.to_string()), Box::new(responder));
    })
}

/// Invokes a virtual canister method. This function is supposed to be called through [virtual_canister_call] macro.
#[doc(hidden)]
pub fn call_virtual_responder(
    principal: Principal,
    method_name: &str,
    args: Vec<u8>,
) -> CallResult<Vec<u8>> {
    __RESPONDERS.with(|responders| {
        match responders
            .borrow()
            .get(&(principal, method_name.to_string()))
        {
            Some(responder) => responder(args),
            None => Err((
                RejectionCode::Unknown,
                format!(
                    "canister method {method_name} is not registered for principal {principal}, METHODS: {:?}",
                    responders
                        .borrow()
                        .keys()
                        .map(|(k, r)| (k.to_string(), r))
                        .collect::<Vec<_>>()
                ),
            )),
        }
    })
}

/// Saves a function that will be called when testing inter-canister calls, invoked with
/// [virtual_canister_call] macro.
///
/// One function can be registered for every principal-method pair. If a `virtual_canister_call` is
/// called in testing environment without a registered responder, an error will be returned. This can
/// be used to test for not existing canisters.
pub fn register_virtual_responder<F, T, U>(principal: Principal, method: &str, closure: F)
where
    F: Fn(T) -> U + 'static,
    for<'a> T: CandidType + ArgumentDecoder<'a>,
    for<'b> U: CandidType + Deserialize<'b>,
{
    let inner_closure = move |args: Vec<u8>| {
        let deserialized_args = candid::decode_args::<T>(&args).map_err(|e| {
            (
                RejectionCode::Unknown,
                format!("Failed to decode args: {:?}", e),
            )
        })?;
        let result = closure(deserialized_args);
        candid::encode_args((result,)).map_err(|e| {
            (
                RejectionCode::Unknown,
                format!("failed to encode return value: {:?}", e),
            )
        })
    };

    _register_virtual_responder(principal, method, inner_closure);
}

/// Adds a responder function for a [virtual_canister_call] that will result in an error result with
/// the given error message.
pub fn register_failing_virtual_responder(
    principal: Principal,
    method: &str,
    error_message: String,
) {
    _register_virtual_responder(principal, method, move |_| {
        Err((RejectionCode::Unknown, error_message.clone()))
    });
}
