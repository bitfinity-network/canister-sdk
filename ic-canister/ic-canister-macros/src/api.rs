use lazy_static::lazy_static;
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use std::collections::BTreeMap;
use std::sync::Mutex;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, Error, FnArg, Ident, ImplItemMethod, Pat, PatIdent, PatTuple, ReturnType,
    Signature, Type, TypeTuple, Visibility,
};

pub(crate) fn api_method(
    method_type: &str,
    _attr: TokenStream,
    item: TokenStream,
    is_management_api: bool,
) -> TokenStream {
    let input = parse_macro_input!(item as ImplItemMethod);
    let method = &input.sig.ident;
    if matches!(input.vis, Visibility::Public(_)) {
        panic!(
            "Canister methods should not be public. Check declaration for the method `{method}`."
        );
    }

    if let Err(e) = store_candid_definitions(method_type, &input.sig) {
        return e.to_compile_error().into();
    }

    let method_name = method.to_string();
    let export_name = if !is_management_api {
        format!("canister_{method_type} {method_name}")
    } else {
        format!("canister_{method_type}")
    };

    let internal_method = Ident::new(&format!("__{method_name}"), method.span());

    let internal_method_notify = Ident::new(&format!("___{method_name}"), method.span());

    let return_type = &input.sig.output;
    let reply_call = if is_management_api {
        if *return_type != ReturnType::Default {
            panic!("{method_type} method cannot have a return type.");
        }

        quote! {}
    } else {
        match return_type {
            ReturnType::Default => quote! {::ic_cdk::api::call::reply(())},
            ReturnType::Type(_, t) => match t.as_ref() {
                Type::Tuple(_) => quote! {::ic_cdk::api::call::reply(result)},
                _ => quote! {::ic_cdk::api::call::reply((result,))},
            },
        }
    };

    let inner_return_type = match return_type {
        ReturnType::Default => quote! {()},
        ReturnType::Type(_, t) => quote! {#t},
    };

    let args = &input.sig.inputs;
    let mut arg_types = Punctuated::new();
    let mut args_destr = Punctuated::new();
    let mut has_self = false;

    for arg in args {
        let (arg_type, arg_pat) = match arg {
            FnArg::Receiver(_) => {
                has_self = true;
                continue;
            }
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

    if !has_self {
        return TokenStream::from(
            syn::Error::new(input.span(), "API method must have a `&self` argument")
                .to_compile_error(),
        );
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
        #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
        #input

        #[cfg(all(target_arch = "wasm32", not(feature = "no_api")))]
        #[export_name = #export_name]
        fn #internal_method() {
            ::ic_cdk::setup();
            ::ic_cdk::spawn(async {
                let #args_destr_tuple: #arg_type = ::ic_cdk::api::call::arg_data();
                let mut instance = Self::init_instance();
                let result = instance. #method(#args_destr) #await_call;
                #reply_call
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        #[allow(unused_mut)]
        pub async fn #internal_method(#args) -> ::ic_cdk::api::call::CallResult<#inner_return_type> {
            // todo: trap handler
            Ok(self. #method(#args_destr) #await_call)
        }

        #[cfg(not(target_arch = "wasm32"))]
        #[allow(unused_mut)]
        #[allow(unused_must_use)]
        pub fn #internal_method_notify(#args) -> Result<(), ::ic_cdk::api::call::RejectionCode> {
            // todo: trap handler
            self. #method(#args_destr);
            Ok(())
        }
    };

    TokenStream::from(expanded)
}

struct Method {
    args: Vec<String>,
    rets: Vec<String>,
    modes: String,
}

// There is no official way to communicate information across proc macro invocations.
// lazy_static works for now, but may get incomplete info with incremental compilation.
// See https://github.com/rust-lang/rust/issues/44034
// Hopefully, we can have an attribute on impl, then we don't need global state.
lazy_static! {
    static ref METHODS: Mutex<BTreeMap<String, Method>> = Mutex::new(Default::default());
    static ref INIT: Mutex<Option<Vec<String>>> = Mutex::new(None);
}

fn store_candid_definitions(modes: &str, sig: &Signature) -> Result<(), syn::Error> {
    if !sig.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &sig.generics,
            "candid_method doesn't support generic parameters",
        ));
    }
    let name = sig.ident.to_string();

    let (args, rets) = get_args(sig)?;

    let args: Vec<String> = args
        .iter()
        .map(|t| format!("{}", t.to_token_stream()))
        .collect();

    let rets: Vec<String> = rets
        .iter()
        .map(|t| format!("{}", t.to_token_stream()))
        .collect();

    if modes == "oneway" && !rets.is_empty() {
        return Err(Error::new_spanned(
            &sig.output,
            "oneway function should have no return value",
        ));
    }

    // Insert init
    if modes == "init" && !rets.is_empty() {
        return Err(Error::new_spanned(
            &sig.output,
            "init method should have no return value or return Self",
        ));
    }

    if modes == "init" {
        match &mut *INIT.lock().unwrap() {
            Some(_) => return Err(Error::new_spanned(&sig.ident, "duplicate init method")),
            ret @ None => *ret = Some(args),
        }
        return Ok(());
    }

    // Insert method
    let mut map = METHODS.lock().unwrap();

    if map.contains_key(&name) {
        return Err(Error::new_spanned(
            &name,
            format!("duplicate method name {name}"),
        ));
    }

    let method = Method {
        args,
        rets,
        modes: modes.to_string(),
    };

    map.insert(name, method);

    Ok(())
}

pub(crate) fn generate_idl() -> TokenStream {
    let candid = quote! { ::ic_cdk::export::candid };

    // Init
    let init = INIT.lock().unwrap().as_mut().map(|args| {
        let args = args
            .drain(..)
            .map(|t| generate_arg(quote! { init_args }, &t))
            .collect::<Vec<_>>();

        let res = quote! {
            let mut init_args = Vec::new();
            #(#args)*
        };

        res
    });

    // Methods
    let mut meths = METHODS.lock().unwrap();

    let gen_tys = meths.iter().map(|(name, Method { args, rets, modes })| {
        let args = args
            .iter()
            .map(|t| generate_arg(quote! { args }, t))
            .collect::<Vec<_>>();

        let rets = rets
            .iter()
            .map(|t| generate_arg(quote! { rets }, t))
            .collect::<Vec<_>>();

        let modes = match modes.as_ref() {
            "query" => quote! { vec![#candid::parser::types::FuncMode::Query] },
            "oneway" => quote! { vec![#candid::parser::types::FuncMode::Oneway] },
            "update" => quote! { vec![] },
            _ => unreachable!(),
        };

        quote! {
            {
                let mut args = Vec::new();
                #(#args)*
                let mut rets = Vec::new();
                #(#rets)*
                let func = Function { args, rets, modes: #modes };
                service.push((#name.to_string(), Type::Func(func)));
            }
        }
    });

    let service = quote! {
        use #candid::types::{CandidType, Function, Type};
        let mut service = Vec::<(String, Type)>::new();
        let mut env = #candid::types::internal::TypeContainer::new();
        #(#gen_tys)*
        service.sort_unstable_by_key(|(name, _)| name.clone());
        let ty = Type::Service(service);
    };

    meths.clear();

    let actor = match init {
        Some(init) => quote! {
            #init
            let actor = Some(Type::Class(init_args, Box::new(ty)));
        },
        None => quote! { let actor = Some(ty); },
    };

    let res = quote! {
        {
            fn __export_service() -> String {
                #service
                #actor
                let result = #candid::bindings::candid::compile(&env.env, &actor);
                format!("{}", result)
            }

            __export_service()
        }
    };

    TokenStream::from(res)
}

fn generate_arg(name: proc_macro2::TokenStream, ty: &str) -> proc_macro2::TokenStream {
    let ty = syn::parse_str::<Type>(ty).unwrap();
    quote! {
        #name.push(env.add::<#ty>());
    }
}

fn get_args(sig: &Signature) -> Result<(Vec<Type>, Vec<Type>), Error> {
    let mut args = Vec::new();
    for arg in &sig.inputs {
        match arg {
            syn::FnArg::Receiver(r) => {
                if r.reference.is_none() {
                    return Err(Error::new_spanned(
                        arg,
                        "cannot take `self` by value, consider borrowing the value: `&self`",
                    ));
                }
            }
            syn::FnArg::Typed(syn::PatType { ty, .. }) => args.push(ty.as_ref().clone()),
        }
    }
    let rets = match &sig.output {
        ReturnType::Default => Vec::new(),
        ReturnType::Type(_, ty) => match ty.as_ref() {
            Type::Tuple(tuple) => tuple.elems.iter().cloned().collect(),
            _ => vec![ty.as_ref().clone()],
        },
    };
    Ok((args, rets))
}
