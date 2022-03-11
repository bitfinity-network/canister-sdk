use proc_macro::TokenStream;
use quote::quote;
use syn::{FnArg, parse_macro_input, Pat, PatIdent, PatTuple, ReturnType, Type, TypeTuple, Visibility, Ident, ImplItemMethod};
use syn::punctuated::Punctuated;

pub(crate) fn api_method(method_type: &str, _attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ImplItemMethod);
    let method = &input.sig.ident;
    if matches!(input.vis, Visibility::Public(_)) {
        panic!(
            "Canister methods should not be public. Check declaration for the method `{method}`."
        );
    }

    let method_name = method.to_string();
    let export_name = format!("canister_{method_type} {method_name}");
    let internal_method = Ident::new(&format!("__{method_name}"), method.span());

    let return_type = &input.sig.output;
    let reply_call = match return_type {
        ReturnType::Default => quote! {::ic_cdk::api::call::reply(())},
        ReturnType::Type(_, t) => match t.as_ref() {
            Type::Tuple(_) => quote! {::ic_cdk::api::call::reply(result)},
            _ => quote! {::ic_cdk::api::call::reply((result,))},
        },
    };

    let inner_return_type = match return_type {
        ReturnType::Default => quote! {()},
        ReturnType::Type(_, t) => quote! {#t},
    };

    let args = &input.sig.inputs;
    let mut arg_types = Punctuated::new();
    let mut args_destr = Punctuated::new();
    for arg in args {
        let (arg_type, arg_pat) = match arg {
            FnArg::Receiver(_) => continue,
            FnArg::Typed(t) => (&t.ty, t.pat.as_ref()),
        };

        let arg_name = match arg_pat {
            Pat::Ident(x) => &x.ident,
            _ => panic!("Invalid arg name"),
        };

        arg_types.push_value(arg_type.as_ref().clone());
        arg_types.push_punct(Default::default());

        let ident = PatIdent {
            attrs: vec![],
            by_ref: None,
            mutability: None,
            ident: arg_name.clone(),
            subpat: None,
        };
        args_destr.push_value(Pat::Ident(ident));
        args_destr.push_punct(Default::default());
    }

    let arg_type = TypeTuple {
        paren_token: Default::default(),
        elems: arg_types,
    };

    let args_destr_tuple = PatTuple {
        attrs: vec![],
        paren_token: Default::default(),
        elems: args_destr.clone(),
    };

    let await_call = if input.sig.asyncness.is_some() {
        quote! { .await }
    } else {
        quote! {}
    };

    let expanded = quote! {
        #input

        #[cfg(all(target_arch = "wasm32"))]
        #[export_name = #export_name]
        fn #internal_method() {
            ::ic_cdk::setup();
            ::ic_cdk::block_on(async {
                let #args_destr_tuple: #arg_type = ::ic_cdk::api::call::arg_data();
                let mut instance = Self::init_instance();
                let result = instance. #method(#args_destr) #await_call;
                #reply_call
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        pub async fn #internal_method(#args) -> ::ic_cdk::api::call::CallResult<#inner_return_type> {
            // todo: trap handler
            Ok(self. #method(#args_destr) #await_call)
        }
    };

    TokenStream::from(expanded)
}
