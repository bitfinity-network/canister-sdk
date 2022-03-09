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

/// Marks the canister method as an API query method.
#[proc_macro_attribute]
pub fn query(_attr: TokenStream, item: TokenStream) -> TokenStream {
    api::api_method("query", _attr, item)
}

/// Marks the canister method as an API update method.
#[proc_macro_attribute]
pub fn update(_attr: TokenStream, item: TokenStream) -> TokenStream {
    api::api_method("update", _attr, item)
}

/// Derives [Canister] trait for a struct.
#[proc_macro_derive(Canister, attributes(id, state))]
pub fn derive_canister(input: TokenStream) -> TokenStream {
    derive::derive_canister(input)
}
