use proc_macro::TokenStream;
use quote::quote;
use syn::{ExprMethodCall, parse_macro_input, Token, Type, TypeTuple, Ident};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;

struct CanisterCall {
    method_call: ExprMethodCall,
    response_type: Type,
}

impl Parse for CanisterCall {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let method_call = input.parse()?;
        input.parse::<Token![,]>()?;
        let response_type = input.parse()?;
        Ok(Self {
            method_call,
            response_type,
        })
    }
}

pub fn canister_call(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as CanisterCall);

    let canister = input.method_call.receiver;
    let method = input.method_call.method;
    let method_name = method.to_string();
    let inner_method = Ident::new(&format!("__{method}"), method.span());
    let mut args = input.method_call.args;
    if !args.empty_or_trailing() {
        args.push_punct(Default::default());
    }

    let response_type = input.response_type;
    let is_tuple = matches!(response_type, Type::Tuple(_));

    let cdk_call = if is_tuple {
        quote! {
            ::ic_cdk::api::call::call::<_, #response_type>(#canister.principal(), #method_name, (#args))
        }
    } else {
        let mut elems = Punctuated::new();
        elems.push_value(response_type);
        elems.push_punct(Default::default());
        let tuple_response_type = Type::Tuple(TypeTuple {
            paren_token: Default::default(),
            elems,
        });

        quote! {
            async {::ic_cdk::api::call::call::<_, #tuple_response_type>(#canister.principal(), #method_name, (#args)).await.map(|x| x.0)}
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
                #canister.#inner_method(#args)
            }
        }
    };

    TokenStream::from(expanded)
}
