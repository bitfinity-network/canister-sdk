use proc_macro::TokenStream;

mod api;
mod canister_call;
mod derive;

/// Makes an inter-canister call. This macro takes two inputs: the canister method invocation,
/// and the expected return type. The result type of invocation is `async CallResult`:
///
/// ```ignore
/// let result: ic_cdk::api::call::CallResult<ResultType> = canister_call!(canister_instance.method_name(arg1, arg2), ReturnType).await;
/// ```
///
/// To obtain a canister instance for this call, use [`ic_canister::Canister::from_principal`] method.
/// If the canister to be called does not implement [`ic_canister::Canister`] trait, use
/// [`virtual_canister_call`] macro instead.
#[proc_macro]
pub fn canister_call(input: TokenStream) -> TokenStream {
    canister_call::canister_call(input)
}

/// Makes an inter-canister call, which sends a one-way message. This macro is the same as [`canister_call`] usage, except ignoring the reply.
/// 
/// Returns `Ok(())` if the message was successfully enqueued, otherwise returns a reject code.
/// 
/// ```ignore
/// let result: Result<(), ic_cdk::api::call::RejectionCode> = canister_notify!(canister_instance.method_name(arg1, arg2), ());
/// ```
/// 
/// To obtain a canister instance for this call, use [`ic_canister::Canister::from_principal`] method.
/// If the canister to be called does not implement [`ic_canister::Canister`] trait, use
/// [`virtual_canister_notify`] macro instead.
/// 
///  # Notes
///
///   * The caller has no way of checking whether the destination processed the notification.
///     The system can drop the notification if the destination does not have resources to
///     process the message (for example, if it's out of cycles or queue slots).
///
///   * The callee cannot tell whether the call is one-way or not.
///     The callee must produce replies for all incoming messages.
///
///   * It is safe to upgrade a canister without stopping it first if it sends out *only*
///     one-way messages.
///
///   * If the payment is non-zero and the system fails to deliver the notification, the behaviour
///     is unspecified: the funds can be either reimbursed or consumed irrevocably by the IC depending
///     on the underlying implementation of one-way calls.
#[proc_macro]
pub fn canister_notify(input: TokenStream) -> TokenStream {
    canister_call::canister_notify(input)
}

/// Makes an inter-canister call to a canister, that has no `Canister` trait implementation.
///
/// ```ignore
/// let result: ic_cdk::api::call::CallResult<ResultType> = virtual_canister_call!(canister_principal, "method_name", (arg1, arg2), ReturnType).await;
/// ```
///
/// To test canister logic that uses such inter-canister calls, one should use `ic_canister::register_virtual_responder`
/// function beforehand to set the function, that will generate responses for the inter-canister
/// calls.
#[proc_macro]
pub fn virtual_canister_call(input: TokenStream) -> TokenStream {
    canister_call::virtual_canister_call(input)
}

/// Makes an inter-canister call to a canister, which sends a one-way message, when has no `Canister` trait implementation.
///
/// ```ignore
/// let result: Result<(), ic_cdk::api::call::RejectionCode> = virtual_canister_notify!(canister_principal, "method_name", (arg1, arg2), ());
/// ```
///
/// To test canister logic that uses such inter-canister calls, one should use `ic_canister::register_virtual_responder`
/// function beforehand to set the function, that will generate responses for the inter-canister
/// calls.
#[proc_macro]
pub fn virtual_canister_notify(input: TokenStream) -> TokenStream {
    canister_call::virtual_canister_notify(input)
}

/// Marks the canister method as an `init` method.
///
/// Only one method in a canister can be marked as `#[init]`. This method must not have a return value.
///
/// This macro also registers the method for generating IDL (candid) definition with [`generate_idl`]
/// function. Thus, there's no need to mark it with `candid::candid_method` macro.
#[proc_macro_attribute]
pub fn init(attr: TokenStream, item: TokenStream) -> TokenStream {
    api::api_method("init", attr, item, true)
}

/// Marks the canister method as an API query method.
///
/// This macro also registers the method for generating IDL (candid) definition with [`generate_idl`]
/// function. Thus, there's no need to mark it with `candid::candid_method` macro.
#[proc_macro_attribute]
pub fn query(attr: TokenStream, item: TokenStream) -> TokenStream {
    api::api_method("query", attr, item, false)
}

/// Marks the canister method as an API update method.
///
/// This macro also registers the method for generating IDL (candid) definition with [`generate_idl`]
/// function. Thus, there's no need to mark it with `candid::candid_method` macro.
#[proc_macro_attribute]
pub fn update(attr: TokenStream, item: TokenStream) -> TokenStream {
    api::api_method("update", attr, item, false)
}

/// Generates IDL (Candid) definition of the canister.
///
/// ```ignore
/// use ic_cdk::export::Principal;
/// use ic_canister::Canister;
///
/// #[derive(Clone, Canister)]
/// struct MyCanister {
///     #[id]
///     principal: Principal,
/// }
///
/// assert_eq!(generate_idl!(), "service: () {}".to_string());
/// ```
#[proc_macro]
pub fn generate_idl(_: TokenStream) -> TokenStream {
    api::generate_idl()
}

/// Derives [Canister] trait for a struct.
#[proc_macro_derive(Canister, attributes(id, state, trait_name))]
pub fn derive_canister(input: TokenStream) -> TokenStream {
    derive::derive_canister(input)
}
