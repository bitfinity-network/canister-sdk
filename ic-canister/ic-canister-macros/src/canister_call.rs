use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    parse_macro_input, Expr, ExprMethodCall, ExprTuple, Ident, LitStr, Token, Type, TypeTuple,
};

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
    let args = normalize_args(&input.method_call.args);
    let cycles = input.cycles;
    let cdk_call = get_cdk_call(
        quote! {#canister.principal()},
        &method_name,
        &args,
        &input.response_type,
        cycles,
    );

    let expanded = quote! {
        {
            #[cfg(target_arch = "wasm32")]
            {
                #cdk_call
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                #canister.#inner_method(#args)
            }
        }
    };

    TokenStream::from(expanded)
}

struct VirtualCanisterCall {
    principal: Expr,
    method_name: LitStr,
    args: ExprTuple,
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
    let args = normalize_args(&input.args.elems);
    let method_name = input.method_name.value();
    let response_type = &input.response_type;
    let cycles = input.cycles;

    let cdk_call = get_cdk_call(
        quote! {#principal},
        &method_name,
        &args,
        response_type,
        cycles,
    );

    let is_tuple = matches!(response_type, Type::Tuple(_));

    let (decode, tuple_index) = if is_tuple {
        (
            quote! { ::ic_cdk::export::candid::decode_args::<#response_type>(&result) },
            quote! {},
        )
    } else {
        (
            quote! { ::ic_cdk::export::candid::decode_args::<(#response_type,)>(&result) },
            quote! {.0},
        )
    };

    let responder_call = quote! {
        async {
            let encoded_args = match ::ic_cdk::export::candid::encode_args((#args)) {
                Ok(v) => v,
                Err(e) => return Err((::ic_cdk::api::call::RejectionCode::Unknown, format!("failed to serialize arguments: {}", e))),
            };

            let result = ::ic_canister::call_virtual_responder(#principal, #method_name, encoded_args)?;

            let result = match #decode {
                Ok(v) => v #tuple_index,
                Err(e) => return Err((::ic_cdk::api::call::RejectionCode::Unknown, format!("failed to deserialize return value: {}", e))),
            };

            Ok(result)
        }
    };

    let expanded = quote! {
        {
            #[cfg(target_arch = "wasm32")]
            {
                #cdk_call
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                #responder_call
            }
        }
    };

    TokenStream::from(expanded)
}

fn get_cdk_call(
    principal: proc_macro2::TokenStream,
    method_name: &str,
    args: &Punctuated<Expr, Token![,]>,
    response_type: &Type,
    cycles: Option<Expr>,
) -> proc_macro2::TokenStream {
    let is_tuple = matches!(response_type, Type::Tuple(_));

    if is_tuple {
        if let Some(cycles) = cycles {
            quote! {
                ::ic_cdk::api::call::call_with_payment::<_, #response_type>(#principal, #method_name, (#args), #cycles)
            }
        } else {
            quote! {
                ::ic_cdk::api::call::call::<_, #response_type>(#principal, #method_name, (#args))
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
                    ::ic_cdk::api::call::call_with_payment::<_, #tuple_response_type>(#principal, #method_name, (#args), #cycles).await.map(|x| x.0)
                }
            }
        } else {
            quote! {
                async {
                    ::ic_cdk::api::call::call::<_, #tuple_response_type>(#principal, #method_name, (#args)).await.map(|x| x.0)
                }
            }
        }
    }
}

fn normalize_args(args: &Punctuated<Expr, Token![,]>) -> Punctuated<Expr, Token![,]> {
    let mut args = args.clone();
    if !args.empty_or_trailing() {
        args.push_punct(std::default::Default::default());
    }

    args
}
