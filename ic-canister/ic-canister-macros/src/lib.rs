use proc_macro::TokenStream;

mod api;
mod canister_call;
mod derive;

/// Makes an inter-canister call. This macro takes two inputs: the canister method invocation,
/// and the expected return type.
///
/// ```ignore
/// let result: ic_cdk::api::call::CallResult<ResultType> = canister_call!(canister_instance.method_name(arg1, arg2), ReturnType).await;
/// ```
#[proc_macro]
pub fn canister_call(input: TokenStream) -> TokenStream {
    canister_call::canister_call(input)
}

/// Makes an inter-canister call to a canister, that has no `Canister` trait implementation.
#[proc_macro]
pub fn virtual_canister_call(input: TokenStream) -> TokenStream {
    canister_call::virtual_canister_call(input)
}

/// Marks the canister method as an `init` method.
///
/// Only one method in a canister can be marked as `#[init]`. This method must not have a return value.
#[proc_macro_attribute]
pub fn init(attr: TokenStream, item: TokenStream) -> TokenStream {
    api::api_method("init", attr, item, true)
}

/// Marks the canister method as an API query method.
#[proc_macro_attribute]
pub fn query(attr: TokenStream, item: TokenStream) -> TokenStream {
    api::api_method("query", attr, item, false)
}

/// Marks the canister method as an API update method.
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
