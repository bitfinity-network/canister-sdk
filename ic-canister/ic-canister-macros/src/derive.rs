use std::collections::HashSet;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Field, Fields, GenericArgument, Lit, LitBool,
    Meta, NestedMeta, Path, PathArguments, Type,
};

pub fn derive_canister(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let trait_stream = TokenStream::from(quote! {Canister});
    let trait_name = parse_macro_input::parse::<Path>(trait_stream)
        .expect("static value parsing always succeeds");

    let derive_upgrade = derive_upgrade_methods(&input);

    let name = input.ident;

    let data = match input.data {
        Data::Struct(v) => v,
        _ => panic!("canister trait can only be derived for a structure"),
    };

    let fields = match data.fields {
        Fields::Named(v) => v,
        _ => panic!("canister derive is not supported for tuple-like structs"),
    }
    .named;

    let mut principal_fields = vec![];
    let mut state_fields = vec![];
    let mut default_fields = vec![];
    'field: for field in fields {
        for attr in field.attrs.iter() {
            match attr {
                attr if is_principal_attr(attr) => {
                    principal_fields.push(field.clone());
                    continue 'field;
                }
                attr if is_state_attr(attr) => {
                    state_fields.push(field.clone());
                    continue 'field;
                }
                _ => continue,
            }
        }

        default_fields.push(field);
    }

    if principal_fields.len() != 1 {
        panic!("canister struct must contains exactly one `id` field");
    }

    let principal_field = principal_fields
        .remove(0)
        .ident
        .expect("at structure declaration there can be no field with name");

    let mut used_types = HashSet::new();
    let state_fields = state_fields.iter().map(|field| {
        let field_name = field.ident.clone().expect("Fields always have name");
        let field_type = get_state_type(&field.ty);

        if !used_types.insert(field_type) {
            panic!("canister cannot have two fields with the type {field_type:?}",);
        }

        let is_stable = is_state_field_stable(field);
        (field_name, field_type, is_stable)
    });

    let mut stable_field = None;
    let state_fields_wasm = if state_fields.len() > 0 {
        let mut state_fields_wasm = vec![];

        for (field_name, field_type, is_stable) in state_fields {
            state_fields_wasm
                .push(quote! {#field_name : <#field_type as ic_storage::IcStorage>::get()});

            if is_stable {
                match stable_field {
                    None => stable_field = Some((field_name, field_type)),
                    Some(_) => panic!("only one state field can have the `stable_storage` flag"),
                }
            }
        }

        quote! {, #(#state_fields_wasm),* }
    } else {
        quote! {}
    };

    let default_fields = default_fields.iter().map(|x| {
        let field_name = x.ident.clone().unwrap();
        let field_type = &x.ty;
        quote! {#field_name : <#field_type as ::std::default::Default>::default()}
    });

    let default_fields = if default_fields.len() > 0 {
        quote! {, #(#default_fields),* }
    } else {
        quote! {}
    };

    let upgrade_methods = if derive_upgrade {
        expand_upgrade_methods(&name, stable_field)
    } else {
        quote! {}
    };

    let expanded = quote! {
        #[cfg(not(target_family = "wasm"))]
        thread_local! {
            static __NEXT_ID: ::std::sync::atomic::AtomicU64 = {
                let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
                let id: u64 = (nanos % 10u128.pow(19)).try_into().unwrap();
                id.into()
            };
        }

        #[cfg(not(target_family = "wasm"))]
        fn __next_id() -> [u8; 8] {
            __NEXT_ID.with(|v| v.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed).to_le_bytes())
        }

        #[automatically_derived]
        impl #trait_name for #name {
            #[cfg(target_family = "wasm")]
            fn init_instance() -> Self {
                let principal = ::ic_exports::ic_cdk::api::id();
                Self { #principal_field : principal #state_fields_wasm #default_fields }
            }

            #[cfg(not(target_family = "wasm"))]
            fn init_instance() -> Self {
                let principal = ::ic_exports::candid::Principal::from_slice(&__next_id());
                Self::from_principal(principal)
            }

            #[cfg(target_family = "wasm")]
            fn from_principal(principal: ::ic_exports::candid::Principal) -> Self {
                Self { #principal_field: principal #state_fields_wasm #default_fields }
            }

            #[cfg(not(target_family = "wasm"))]
            fn from_principal(principal: ::ic_exports::candid::Principal) -> Self {
                let curr_id = ::ic_exports::ic_kit::ic::id();

                // We set the id in the mock context to be the one of new canister to initialize
                // the state of that canister in local storage
                ::ic_exports::ic_kit::inject::get_context().update_id(principal);
                let instance = Self { #principal_field: principal #state_fields_wasm #default_fields };

                // And then we reset the id to what it was
                ::ic_exports::ic_kit::inject::get_context().update_id(curr_id);
                instance
            }

            fn principal(&self) -> Principal {
                self.#principal_field
            }
        }

        #upgrade_methods

    };

    TokenStream::from(expanded)
}

fn expand_upgrade_methods(
    struct_name: &proc_macro2::Ident,
    stable_field: Option<(proc_macro2::Ident, &Type)>,
) -> proc_macro2::TokenStream {
    let (name, field_type) = match stable_field {
        None => return quote!(),
        Some((name, field_type)) => (name, field_type),
    };

    let (state_get, state_borrow) = (
        quote! { let #name = ::std::rc::Rc::clone(&self. #name); },
        quote! { &* #name.borrow(), },
    );

    let field_assignment = quote! { self. #name.replace(#name); };

    quote! {
        impl #struct_name {
            fn __pre_upgrade_inst(&self) {
                use ic_storage::IcStorage;

                #state_get

                ic_storage::stable::write(#state_borrow).unwrap();
            }

            fn __post_upgrade_inst(&self) {
                use ic_storage::IcStorage;
                use ic_storage::stable::Versioned;

                let #name = match ic_storage::stable::read::<#field_type>() {
                    Ok(val) => val,
                    Err(e) => ::ic_exports::ic_cdk::trap(&format!("failed to upgrade: {}", e)),
                };

                #field_assignment
            }

            #[cfg(not(target_family = "wasm"))]
            fn __post_upgrade() {
                let instance = Self::init_instance();
                instance.__post_upgrade_inst();
            }

            #[cfg(not(target_family = "wasm"))]
            fn __pre_upgrade() {
                let instance = Self::init_instance();
                instance.__pre_upgrade_inst();
            }

            #[cfg(all(target_family = "wasm", feature = "export-api"))]
            #[export_name = "canister_pre_upgrade"]
            fn __pre_upgrade() {
                let instance = Self::init_instance();
                instance.__pre_upgrade_inst();
            }

            #[cfg(all(target_family = "wasm", feature = "export-api"))]
            #[export_name = "canister_post_upgrade"]
            fn __post_upgrade() {
                let instance = Self::init_instance();
                instance.__post_upgrade_inst();
            }

        }
    }
}

fn is_state_field_stable(field: &Field) -> bool {
    // Find the "state" field
    let meta = field
        .attrs
        .iter()
        .filter_map(|a| match a.path.get_ident() {
            Some(ident) if ident == "state" => a.parse_meta().ok(),
            _ => None,
        })
        .next();

    let meta_list = match meta {
        Some(Meta::List(list)) => list,
        _ => return true,
    };

    // Since there is only going to be one named value in the args
    // it makes sense to look at the next value as the only value:
    let next_named_val = match meta_list.nested.into_iter().next() {
        Some(NestedMeta::Meta(Meta::NameValue(meta))) => meta,
        Some(_) | None => return true,
    };

    // Ensure that the path is "stable_store"
    match next_named_val.path.get_ident() {
        Some(ident) if ident == "stable_store" => {}
        Some(_) | None => return true,
    }

    !matches!(next_named_val.lit, Lit::Bool(LitBool { value: false, .. }))
}

fn is_principal_attr(attribute: &Attribute) -> bool {
    attribute.path.is_ident("id")
}

fn is_state_attr(attribute: &Attribute) -> bool {
    attribute.path.is_ident("state")
}

pub fn get_state_type(input_type: &Type) -> &Type {
    let ref_cell = extract_generic("Rc", input_type, input_type);
    extract_generic("RefCell", ref_cell, input_type)
}

pub(crate) fn extract_type_if_matches<'a>(type_name: &str, generic_base: &'a Type) -> &'a Type {
    let v = match generic_base {
        Type::Path(v) => v,
        Type::Tuple(_) => return generic_base,
        Type::Reference(r) => match r.elem.as_ref() {
            Type::Path(v) => v,
            // who would even return references to references?
            ty => {
                panic!("Referenced type should be concrete (either generic or primitive): {ty:#?}")
            }
        },
        ty => panic!("Type is not concrete nor is reference: {ty:#?}"),
    };

    let last = v.path.segments.iter().last();

    let last_segment = match last {
        Some(segment) => segment,
        None => panic!("Given type does not have generic parameters: {v:#?}"),
    };

    // In case if the `type_name` does not wrap the `generic_base` we simply return it.
    if last_segment.ident != type_name {
        return generic_base;
    }

    let arg = match &last_segment.arguments {
        PathArguments::AngleBracketed(arg) => arg,
        _ => panic!("Given type does not have generic parameters: {v:#?}"),
    };

    // Get rid of lifetimes
    let args = arg
        .args
        .iter()
        .filter_map(|a| match a {
            g @ GenericArgument::Type(_) => Some(g),
            _ => None,
        })
        .collect::<Vec<_>>();

    if args.len() != 1 {
        panic!("Cannot extract given type since it has multiple generic parameters: {v:#?}");
    }

    match args[0] {
        GenericArgument::Type(t) => t,
        arg => panic!("Generic parameter to a type is not a generic argument: {arg:#?}"),
    }
}

fn extract_generic<'a>(type_name: &str, generic_base: &'a Type, input_type: &'a Type) -> &'a Type {
    let v = match generic_base {
        Type::Path(v) => v,
        _ => state_type_error(input_type),
    };

    let last = v.path.segments.iter().last();

    let last_segment = match last {
        Some(segment) => segment,
        None => state_type_error(input_type),
    };

    if last_segment.ident != type_name {
        state_type_error(input_type);
    }

    let arg = match &last_segment.arguments {
        PathArguments::AngleBracketed(arg) => arg,
        _ => state_type_error(input_type),
    };

    if arg.args.len() != 1 {
        state_type_error(input_type);
    }

    match &arg.args[0] {
        GenericArgument::Type(t) => t,
        _ => state_type_error(input_type),
    }
}

fn state_type_error(input_type: &Type) -> ! {
    panic!("state field type must be Rc<RefCell<T>> where T: IcStorage, but the actual type is {input_type:?}")
}

fn derive_upgrade_methods(input: &DeriveInput) -> bool {
    !input.attrs.iter().any(|x| {
        x.path
            .segments
            .last()
            .map(|last| last.ident == "canister_no_upgrade_methods")
            .unwrap_or(false)
    })
}
