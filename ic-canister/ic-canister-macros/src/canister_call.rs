use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, Expr, ExprMethodCall, Ident, Token, Type, TypeTuple};

struct CanisterCall {
    method_call: ExprMethodCall,
    response_type: Type,
    cycles: Option<Expr>,
}

impl Parse for CanisterCall {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let method_call = input.parse()?;
        input.parse::<Token![,]>().map_err(|e| {
            syn::Error::new(
                e.span(),
                "second parameter is missing, expecting the method return type",
            )
        })?;
        let response_type = input
            .parse()
            .map_err(|e| syn::Error::new(e.span(), "failed to parse method response type"))?;
        let cycles = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            let cycles = input.parse()?;
            Some(cycles)
        } else {
            None
        };
        Ok(Self {
            method_call,
            response_type,
            cycles,
        })
    }
}

pub(crate) fn canister_call(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as CanisterCall);

    let canister = input.method_call.receiver;
    let method = input.method_call.method;
    let method_name = method.to_string();
    let inner_method = Ident::new(&format!("__{method}"), method.span());
    let args = normalize_args(input.method_call.args);
    let cycles = input.cycles;
    let cdk_call = get_cdk_call(
        quote! {#canister.principal()},
        quote! {#method_name},
        quote! {(#args)},
        &input.response_type,
        cycles,
    );

    let expanded = quote! {
        {
            #[cfg(target_family = "wasm")]
            {
                #cdk_call
            }

            #[cfg(not(target_family = "wasm"))]
            async {
                let __caller = ::ic_exports::ic_kit::ic::caller();
                let __id = ::ic_exports::ic_kit::ic::id();
                ::ic_exports::ic_kit::inject::get_context().update_caller(__id);
                ::ic_exports::ic_kit::inject::get_context().update_id(#canister.principal());

                let result = #canister.#inner_method(#args).await;

                ::ic_exports::ic_kit::inject::get_context().update_caller(__caller);
                ::ic_exports::ic_kit::inject::get_context().update_id(__id);
                result
            }
        }
    };

    TokenStream::from(expanded)
}

pub(crate) fn canister_notify(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as CanisterCall);

    let canister = input.method_call.receiver;
    let method = input.method_call.method;
    let method_name = method.to_string();
    let inner_method = Ident::new(&format!("___{method}"), method.span());
    let args = normalize_args(input.method_call.args);
    let cycles = input.cycles;
    let cdk_call = get_cdk_notify(
        quote! {#canister.principal()},
        quote! {#method_name},
        quote! {(#args)},
        cycles,
    );

    let expanded = quote! {
        {
            #[cfg(target_family = "wasm")]
            {
                #cdk_call
            }

            #[cfg(not(target_family = "wasm"))]
            {
                let __caller = ::ic_exports::ic_kit::ic::caller();
                let __id = ::ic_exports::ic_kit::ic::id();
                ::ic_exports::ic_kit::inject::get_context().update_caller(__id);
                ::ic_exports::ic_kit::inject::get_context().update_id(#canister.principal());

                let result = #canister.#inner_method(#args);

                ::ic_exports::ic_kit::inject::get_context().update_caller(__caller);
                ::ic_exports::ic_kit::inject::get_context().update_id(__id);
                result            }
        }
    };

    TokenStream::from(expanded)
}

struct VirtualCanisterCall {
    principal: Expr,
    method_name: Expr,
    args: Expr,
    response_type: Type,
    cycles: Option<Expr>,
}

impl Parse for VirtualCanisterCall {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let principal = input.parse()?;
        input.parse::<Token![,]>()?;

        let method_name = input.parse()?;
        input.parse::<Token![,]>()?;

        let args = input.parse()?;

        input.parse::<Token![,]>()?;
        let response_type = input.parse()?;

        let cycles = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            let cycles = input.parse()?;
            Some(cycles)
        } else {
            None
        };

        Ok(Self {
            principal,
            method_name,
            args,
            response_type,
            cycles,
        })
    }
}

pub(crate) fn virtual_canister_call(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as VirtualCanisterCall);
    let principal = &input.principal;
    let args = normalize_expr(input.args);
    let method_name = &input.method_name;
    let response_type = &input.response_type;
    let cycles = input.cycles;

    let cdk_call = get_cdk_call(
        quote! {#principal},
        quote! {#method_name},
        quote! {#args},
        response_type,
        cycles,
    );

    let is_tuple = matches!(response_type, Type::Tuple(_));

    let (decode, tuple_index) = if is_tuple {
        (
            quote! { ::ic_exports::candid::decode_args::<#response_type>(&result) },
            quote! {},
        )
    } else {
        (
            quote! { ::ic_exports::candid::decode_args::<(#response_type,)>(&result) },
            quote! {.0},
        )
    };

    let responder_call = quote! {
        async {
            let encoded_args = match ::ic_exports::candid::encode_args((#args)) {
                Ok(v) => v,
                Err(e) => return Err(
                    ::ic_exports::ic_cdk::call::Error::from(
                        ::ic_exports::ic_cdk::call::CallFailed::CallRejected(::ic_exports::ic_cdk::call::CallRejected::with_rejection(
                        0,
                        format!("failed to serialize arguments: {e}"),
                )))),
            };

            let __caller = ::ic_exports::ic_kit::ic::caller();
            let __id = ::ic_exports::ic_kit::ic::id();
            ::ic_exports::ic_kit::inject::get_context().update_caller(__id);
            ::ic_exports::ic_kit::inject::get_context().update_id(#principal);

            let result = ::ic_canister::call_virtual_responder(#principal, #method_name, encoded_args);

            ::ic_exports::ic_kit::inject::get_context().update_caller(__caller);
            ::ic_exports::ic_kit::inject::get_context().update_id(__id);

            let result = result?;

            let result = match #decode {
                Ok(v) => v #tuple_index,
                Err(e) => return Err(
                    ::ic_exports::ic_cdk::call::Error::from(
                        ::ic_exports::ic_cdk::call::CallFailed::CallRejected(::ic_exports::ic_cdk::call::CallRejected::with_rejection(
                            0,
                            format!("failed to deserialize arguments: {e}"),
                        )))
                ),
            };
            Ok(result)
        }
    };

    let expanded = quote! {
        {
            #[cfg(target_family = "wasm")]
            {
                #cdk_call
            }

            #[cfg(not(target_family = "wasm"))]
            {
                #responder_call
            }
        }
    };

    TokenStream::from(expanded)
}

pub(crate) fn virtual_canister_notify(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as VirtualCanisterCall);
    let principal = &input.principal;
    let args = normalize_expr(input.args);
    let method_name = &input.method_name;
    let cycles = input.cycles;

    let cdk_call = get_cdk_notify(
        quote! {#principal},
        quote! {#method_name},
        quote! {#args},
        cycles,
    );

    let responder_call = quote! {

        let notify_call = || -> ::ic_exports::ic_cdk::call::CallResult<()> {
            let encoded_args = match ::ic_exports::candid::encode_args((#args)) {
                Ok(v) => v,
                Err(e) => return Err(
                    ::ic_exports::ic_cdk::call::Error::from(
                        ::ic_exports::ic_cdk::call::CallFailed::CallRejected(::ic_exports::ic_cdk::call::CallRejected::with_rejection(
                            0,
                            format!("failed to serialize arguments: {e}"),
                        )))
                ),
            };

            let result = ::ic_canister::call_virtual_responder(#principal, #method_name, encoded_args)?;
            Ok(())
        };
        notify_call()
    };

    let expanded = quote! {
        {
            #[cfg(target_family = "wasm")]
            {
                #cdk_call
            }

            #[cfg(not(target_family = "wasm"))]
            {
                #responder_call
            }
        }
    };

    TokenStream::from(expanded)
}

fn get_cdk_call(
    principal: proc_macro2::TokenStream,
    method_name: proc_macro2::TokenStream,
    args: proc_macro2::TokenStream,
    response_type: &Type,
    cycles: Option<Expr>,
) -> proc_macro2::TokenStream {
    let is_tuple = matches!(response_type, Type::Tuple(_));

    if is_tuple {
        if let Some(cycles) = cycles {
            quote! {
                ::ic_exports::ic_cdk::call::Call::unbounded_wait(#principal, #method_name)
                    .with_args(&#args)
                    .with_cycles(#cycles)
            }
        } else {
            quote! {
                ::ic_exports::ic_cdk::call::Call::unbounded_wait(#principal, #method_name)
                    .with_args(&#args)
            }
        }
    } else {
        let mut elems = Punctuated::new();
        elems.push_value(response_type.clone());
        elems.push_punct(Default::default());
        let tuple_response_type = Type::Tuple(TypeTuple {
            paren_token: Default::default(),
            elems,
        });

        if let Some(cycles) = cycles {
            quote! {
                async {
                    match ::ic_exports::ic_cdk::call::Call::unbounded_wait(#principal, #method_name)
                        .with_args(&#args)
                        .with_cycles(#cycles)
                        .await
                        .map_err(::ic_exports::ic_cdk::call::Error::from)
                    {
                        Ok(r) => r
                            .candid_tuple::<#tuple_response_type>()
                            .map(|r| r.0)
                            .map_err(::ic_exports::ic_cdk::call::Error::from),
                        Err(e) => Err(e),
                    }
                }
            }
        } else {
            quote! {
                async {
                    match ::ic_exports::ic_cdk::call::Call::unbounded_wait(#principal, #method_name)
                        .with_args(&#args)
                        .await
                        .map_err(::ic_exports::ic_cdk::call::Error::from)
                    {
                        Ok(r) => r
                            .candid_tuple::<#tuple_response_type>()
                            .map(|r| r.0)
                            .map_err(::ic_exports::ic_cdk::call::Error::from),
                        Err(e) => Err(e),
                    }
                }
            }
        }
    }
}

fn get_cdk_notify(
    principal: proc_macro2::TokenStream,
    method_name: proc_macro2::TokenStream,
    args: proc_macro2::TokenStream,
    cycles: Option<Expr>,
) -> proc_macro2::TokenStream {
    if let Some(cycles) = cycles {
        quote! {
            ::ic_exports::ic_cdk::call::Call::unbounded_wait(#principal, #method_name)
                .with_args(&#args)
                .with_cycles(#cycles)
                .oneway()
                .map_err(::ic_exports::ic_cdk::call::Error::from)
        }
    } else {
        quote! {
            ::ic_exports::ic_cdk::call::Call::unbounded_wait(#principal, #method_name)
                .with_args(&#args)
                .oneway()
                .map_err(::ic_exports::ic_cdk::call::Error::from)

        }
    }
}

fn normalize_expr(args: Expr) -> Expr {
    match args {
        Expr::Tuple(mut expr) => {
            expr.elems = normalize_args(expr.elems);
            Expr::Tuple(expr)
        }
        _ => args,
    }
}

fn normalize_args(mut args: Punctuated<Expr, Token![,]>) -> Punctuated<Expr, Token![,]> {
    if !args.empty_or_trailing() {
        args.push_punct(std::default::Default::default());
    }

    args
}
