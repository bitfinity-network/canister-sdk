use std::collections::BTreeMap;
use std::sync::Mutex;

use lazy_static::lazy_static;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, ToTokens};
use serde::Deserialize;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, Error, FnArg, Ident, ImplItemMethod, Item, Pat, PatIdent, PatTuple,
    ReturnType, Signature, Stmt, Token, Type, TypeTuple, VisPublic, Visibility,
};

#[derive(Default, Deserialize, Debug)]
struct ApiAttrParameters {
    #[serde(rename = "trait", default)]
    pub is_trait: bool,
}

pub(crate) fn api_method(
    method_type: &str,
    attr: TokenStream,
    item: TokenStream,
    is_management_api: bool,
    with_args: bool,
) -> TokenStream {
    let mut input = parse_macro_input!(item as ImplItemMethod);

    // Insert `pre_update` call before executing the method first
    let method_name = input.sig.ident.to_string();
    if method_type == "update" && method_name != "pre_update" {
        let pre_update_stmt = syn::parse2::<syn::Stmt>(quote! {
            self.pre_update(#method_name, #method_type.into());
        })
        .unwrap();
        input.block.stmts.insert(0, pre_update_stmt);
    }

    let input = input;
    let method = &input.sig.ident;
    let orig_vis = input.vis.clone();

    let parameters =
        serde_tokenstream::from_tokenstream::<ApiAttrParameters>(&attr.into()).unwrap();

    input.sig.generics.params.iter().for_each(|generic| {
        if !matches!(generic, syn::GenericParam::Lifetime(_)) {
            panic!("candid method does not support generics that are not lifetimes");
        }
    });

    if method_type == "init" && parameters.is_trait {
        panic!("Cannot set up init method for a trait definition. This should be done by the struct that implements this trait.");
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
            ReturnType::Default => quote! {::ic_exports::ic_cdk::api::call::reply(())},
            ReturnType::Type(_, t) => match t.as_ref() {
                Type::Tuple(_) => quote! {::ic_exports::ic_cdk::api::call::reply(result)},
                _ => quote! {::ic_exports::ic_cdk::api::call::reply((result,))},
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

    if !with_args && !args_destr.is_empty() {
        return syn::Error::new(
            input.span(),
            format!("{} method cannot have arguments", method_type),
        )
        .to_compile_error()
        .into();
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

    let is_async_return_type = if let ReturnType::Type(_, ty) = &input.sig.output {
        let extracted = crate::derive::extract_type_if_matches("AsyncReturn", ty);
        &**ty != extracted
    } else {
        false
    };

    let await_call = if input.sig.asyncness.is_some() {
        quote! { .await }
    } else {
        quote! {}
    };

    let await_call_if_result_is_async = if is_async_return_type {
        quote! { .await }
    } else {
        quote! {}
    };

    let export_function = if parameters.is_trait {
        let mut methods = METHODS_EXPORTS.lock().unwrap();
        methods.push(ExportMethodData {
            method_name,
            export_name,
            arg_count: args.len(),
            is_async: input.sig.asyncness.is_some(),
            is_return_type_async: is_async_return_type,
            return_type: match return_type {
                ReturnType::Default => ReturnVariant::Default,
                ReturnType::Type(_, t) => match t.as_ref() {
                    Type::Tuple(_) => ReturnVariant::Tuple,
                    _ => ReturnVariant::Type,
                },
            },
        });
        quote! {}
    } else {
        let args_destr_tuple = if with_args {
            quote! {
                let #args_destr_tuple: #arg_type = ::ic_exports::ic_cdk::api::call::arg_data();
            }
        } else {
            quote! {}
        };
        quote! {
            #[cfg(all(target_arch = "wasm32", feature = "export-api"))]
            #[export_name = #export_name]
            fn #internal_method() {
                ::ic_exports::ic_cdk::setup();
                ::ic_exports::ic_cdk::spawn(async {
                    #args_destr_tuple
                    let mut instance = Self::init_instance();
                    let result = instance. #method(#args_destr) #await_call #await_call_if_result_is_async;
                    #reply_call
                });
            }
        }
    };

    let expanded = quote! {
        #[allow(dead_code)]
        #input

        #export_function

        #[cfg(not(target_arch = "wasm32"))]
        #[allow(dead_code)]
        #orig_vis fn #internal_method(#args) -> ::std::pin::Pin<Box<dyn ::core::future::Future<Output = ::ic_exports::ic_cdk::api::call::CallResult<#inner_return_type>> + '_>> {
            // todo: trap handler
            let result = self. #method(#args_destr);
            Box::pin(async move { Ok(result #await_call) })
        }

        #[cfg(not(target_arch = "wasm32"))]
        #[allow(unused_mut)]
        #[allow(unused_must_use)]
        #orig_vis fn #internal_method_notify(#args) -> ::std::result::Result<(), ::ic_exports::ic_cdk::api::call::RejectionCode> {
            // todo: trap handler
            self. #method(#args_destr);
            Ok(())
        }
    };

    TokenStream::from(expanded)
}

#[derive(Debug)]
pub struct StateGetter {
    pub method_name: String,
    pub state_type: String,
}

lazy_static! {
    pub static ref STATE_GETTER: Mutex<Option<StateGetter>> = Mutex::new(None);
}

pub(crate) fn state_getter(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ImplItemMethod);
    let method_name = input.sig.ident.to_string();

    // Check arguments of the getter

    let arg = input.sig.inputs.last();
    match arg {
        Some(FnArg::Receiver(_)) => {}
        _ => {
            return syn::Error::new(
                input.span(),
                "State getter must only have `self` as argument",
            )
            .to_compile_error()
            .into();
        }
    }

    // Check return type of the getter
    let return_type = match &input.sig.output {
        ReturnType::Default => panic!("no return type for state getter is specified"),
        ReturnType::Type(_, t) => crate::derive::get_state_type(t),
    };

    let path = match return_type {
        Type::Path(path) => path,
        ty => {
            return syn::Error::new(
                input.span(),
                format!("invalid return type for state getter: {:#?}", ty),
            )
            .to_compile_error()
            .into()
        }
    };

    let segment = path.path.segments.iter().last();

    let state_type = match segment {
        Some(segment) => segment.ident.to_string(),
        None => {
            return syn::Error::new(
                input.span(),
                format!(
                    "unexpected return type for state getter: {:#?}",
                    return_type
                ),
            )
            .to_compile_error()
            .into()
        }
    };

    // Check that the body of the getter is empty

    let body = &input.block.stmts;

    match &body[..] {
        [Stmt::Item(Item::Verbatim(ts))] if ts.to_string() == ";" => {}
        _ => {
            return syn::Error::new(
                input.span(),
                "State getter must only be defined inside struct implementation and not in trait definition",
            )
            .to_compile_error()
            .into();
        }
    }

    // Replace state getter

    let old_getter = STATE_GETTER.lock().unwrap().replace(StateGetter {
        method_name,
        state_type,
    });

    if let Some(old_getter) = old_getter {
        return syn::Error::new(
            input.span(),
            format!(
                "multiple state getters defined. Previous: {}",
                old_getter.method_name
            ),
        )
        .to_compile_error()
        .into();
    }

    TokenStream::from(quote! { #input })
}

#[derive(Clone)]
enum ReturnVariant {
    Default,
    Type,
    Tuple,
}

#[derive(Clone)]
struct ExportMethodData {
    method_name: String,
    export_name: String,
    arg_count: usize,
    is_async: bool,
    is_return_type_async: bool,
    return_type: ReturnVariant,
}

lazy_static! {
    static ref METHODS_EXPORTS: Mutex<Vec<ExportMethodData>> = Mutex::new(Default::default());
}

struct GenerateExportsInput {
    trait_name: Ident,
    struct_name: Ident,
    struct_vis: Visibility,
}

impl Parse for GenerateExportsInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let trait_name = input.parse::<Ident>()?;
        let (struct_name, struct_vis) = if input.is_empty() {
            (
                Ident::new(&format!("__{}_Ident", trait_name), input.span()),
                Visibility::Inherited,
            )
        } else {
            input.parse::<Token![,]>()?;
            (
                input.parse::<Ident>()?,
                Visibility::Public(VisPublic {
                    pub_token: Default::default(),
                }),
            )
        };

        Ok(Self {
            trait_name,
            struct_name,
            struct_vis,
        })
    }
}

pub(crate) fn generate_exports(input: TokenStream) -> TokenStream {
    let generate_input = parse_macro_input!(input as GenerateExportsInput);
    let GenerateExportsInput {
        trait_name,
        struct_name,
        struct_vis,
    } = generate_input;
    let methods = METHODS_EXPORTS.lock().unwrap();

    let methods = methods.iter().map(|method| {
        let owned: ExportMethodData = method.clone();
        let ExportMethodData { method_name, export_name, arg_count, is_async, is_return_type_async, return_type } = owned;

        let method = Ident::new(&method_name, Span::call_site());
        let internal_method = Ident::new(&format!("__{method}"), Span::call_site());

        // skip first argument as it is always self
        let (args_destr_tuple, args_destr) = if arg_count > 1 {
            let args: Vec<Ident> = (1..arg_count).map(|x| Ident::new(&format!("__arg_{x}"), Span::call_site())).collect();
            (
                quote! { let ( #(#args),* , ) = ::ic_exports::ic_cdk::api::call::arg_data(); },
                quote! { #(#args),* }
            )
        } else {
            (quote! {}, quote! {})
        };

        let await_call = if is_async { quote! {.await}} else {quote! {}};
        let await_call_if_result_is_async = if is_return_type_async { quote! {.await} } else {quote! {}};
        let reply_call = match return_type {
            ReturnVariant::Default => quote! { ::ic_exports::ic_cdk::api::call::reply(()); },
            ReturnVariant::Type => quote! {::ic_exports::ic_cdk::api::call::reply((result,)); },
            ReturnVariant::Tuple => quote! { ::ic_exports::ic_cdk::api::call::reply(result); },
        };

        quote! {
            #[cfg(all(target_arch = "wasm32", feature = "export-api"))]
            #[export_name = #export_name]
            fn #internal_method() {
                ::ic_exports::ic_cdk::setup();
                ::ic_exports::ic_cdk::spawn(async {
                    #args_destr_tuple
                    let mut instance = #struct_name ::init_instance();
                    let result = instance. #method(#args_destr) #await_call #await_call_if_result_is_async;

                    #reply_call
                });
            }
        }
    });

    let state_getter_impl = if let Some(state_getter) = STATE_GETTER.lock().unwrap().take() {
        let state_type = Ident::new(&state_getter.state_type, Span::call_site());
        let method_name = Ident::new(&state_getter.method_name, Span::call_site());

        quote! {
            fn #method_name(&self) -> Rc<RefCell<#state_type>> {
                use ic_storage::IcStorage;
                #state_type::get()
            }
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        #[derive(::std::clone::Clone, ::std::fmt::Debug, Canister)]
        #[allow(non_camel_case_types)]
        #struct_vis struct #struct_name {
            #[id]
            principal: ::ic_exports::candid::Principal,
        }

        impl #trait_name for #struct_name {
            #state_getter_impl
        }

        impl PreUpdate for #struct_name {}

        #(#methods)*
    };
    expanded.into()
}

#[derive(Debug, Clone)]
pub struct Method {
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

    if modes == "pre_upgrade" || modes == "post_upgrade" {
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
    let candid = quote! { ::ic_exports::candid };

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

    let mut methods = METHODS.lock().unwrap();
    let gen_tys = methods.iter().map(|(name, Method { args, rets, modes })| {
        let args = args
            .iter()
            .map(|t| generate_arg(quote! { args }, t))
            .collect::<Vec<_>>();

        let rets = rets
            .iter()
            .map(|t| generate_arg(quote! { rets }, t))
            .collect::<Vec<_>>();

            let modes = match modes.as_ref() {
            "query" => quote! { vec![#candid::types::FuncMode::Query] },
            "oneway" => quote! { vec![#candid::types::FuncMode::Oneway] },
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
                if cfg!(feature = "export-api") {
                    service.push((#name.to_string(), Type(std::rc::Rc::new(TypeInner::Func(func)))));
                }
            }
        }
    });

    let service = quote! {
        use #candid::types::*;
        let mut service = Vec::<(String, Type)>::new();
        let mut env = #candid::types::internal::TypeContainer::new();
        #(#gen_tys)*
        service.sort_unstable_by_key(|(name, _)| name.clone());
        let ty = Type(std::rc::Rc::new(TypeInner::Service(service)));
    };

    methods.clear();

    let actor = match init {
        Some(init) => quote! {
            #init
            let actor = Type(std::rc::Rc::new(TypeInner::Class(init_args, ty)));
        },
        None => quote! { let actor = ty; },
    };

    let res = quote! {
        {
            #service
            #actor
            Idl::new(env, actor)
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
            ty => {
                // Some types in trait canisters had to be marked as `AsyncReturn` as implementation detail
                // but we do not need this when exporting them to candid files as ic calls them correctly
                // in any case.
                let extracted_type = crate::derive::extract_type_if_matches("AsyncReturn", ty);
                vec![extracted_type.clone()]
            }
        },
    };
    Ok((args, rets))
}
