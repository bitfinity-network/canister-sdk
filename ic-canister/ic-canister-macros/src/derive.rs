use proc_macro::TokenStream;
use quote::quote;
use std::collections::HashSet;
use syn::parse::{Parse, ParseStream};
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Field, Fields, GenericArgument, Path,
    PathArguments, Type,
};

#[derive(Debug)]
struct TraitNameAttr {
    path: Path,
}

impl Parse for TraitNameAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let path = input.parse()?;
        Ok(Self { path })
    }
}

impl Default for TraitNameAttr {
    fn default() -> Self {
        let tokens = TokenStream::from(quote! {::ic_canister::Canister});
        let path = parse_macro_input::parse::<Path>(tokens)
            .expect("Static value parsing. Always succeeds.");
        Self { path }
    }
}

pub fn derive_canister(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let trait_name_attr = input.attrs.iter().find(|x| {
        x.path
            .segments
            .last()
            .map(|last| last.ident == "trait_name")
            .unwrap_or(false)
    });

    let trait_attr = match trait_name_attr {
        Some(v) => v.parse_args().expect(
            "Invalid trait_name attribute syntax. It should be `#[trait_name(path::to::Canister)]`",
        ),
        None => TraitNameAttr::default(),
    };

    let trait_name = trait_attr.path;

    let data = match input.data {
        Data::Struct(v) => v,
        _ => panic!("Canister trait can only be derived for a structure."),
    };

    let fields = match data.fields {
        Fields::Named(v) => v,
        _ => panic!("Canister derive is not supported for tuple-like structs."),
    }
    .named;

    let mut principal_fields = vec![];
    let mut state_fields = vec![];
    let mut default_fields = vec![];
    for field in fields {
        if field.attrs.iter().any(is_principal_attr) {
            principal_fields.push(field);
        } else if field.attrs.iter().any(is_state_attr) {
            state_fields.push(field);
        } else {
            default_fields.push(field);
        }
    }

    if principal_fields.len() != 1 {
        panic!("Canister struct must contains exactly one `id` field.");
    }

    let principal_field = principal_fields
        .remove(0)
        .ident
        .expect("At structure declaration there can be no field with name.");

    let mut used_types = HashSet::new();
    let state_fields = state_fields.iter().map(|field| {
        let field_name = field.ident.clone().expect("Fields always have name.");
        let field_type = get_state_type(&field.ty);

        if !used_types.insert(field_type) {
            panic!("Canister cannot have two fields with the type {field_type:?}",);
        }

        let is_stable = is_state_field_stable(field);
        (field_name, field_type, is_stable)
    });

    let mut stable_fields = vec![];
    let (state_fields_wasm, state_fields_test) = if state_fields.len() > 0 {
        let mut state_fields_wasm = vec![];
        let mut state_fields_test = vec![];
        for (field_name, field_type, is_stable) in state_fields {
            state_fields_wasm
                .push(quote! {#field_name : <#field_type as ::ic_storage::IcStorage>::get()});
            state_fields_test.push(quote! {#field_name : ::std::rc::Rc::new(::std::cell::RefCell::new(<#field_type as ::std::default::Default>::default()))});

            if is_stable {
                stable_fields.push((field_name, field_type));
            }
        }
        (
            quote! {, #(#state_fields_wasm),* },
            quote! {, #(#state_fields_test),* },
        )
    } else {
        (quote! {}, quote! {})
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

    let upgrade_methods = expand_upgrade_methods(&name, &stable_fields);

    let expanded = quote! {
        #[cfg(not(target_arch = "wasm32"))]
        thread_local! {
            static CANISTERS: ::std::rc::Rc<::std::cell::RefCell<::std::collections::HashMap<Principal, #name>>> = ::std::rc::Rc::new(::std::cell::RefCell::new(::std::collections::HashMap::new()));
            static __NEXT_ID: ::std::sync::atomic::AtomicU64 = 5.into();
        }

        #[cfg(not(target_arch = "wasm32"))]
        fn __next_id() -> [u8; 8] {
            __NEXT_ID.with(|v| v.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed).to_le_bytes())
        }

        impl #trait_name for #name {
            #[cfg(target_arch = "wasm32")]
            fn init_instance() -> Self {
                Self { #principal_field : ::ic_cdk::export::Principal::anonymous() #state_fields_wasm #default_fields }
            }

            #[cfg(not(target_arch = "wasm32"))]
            fn init_instance() -> Self {
                let instance = Self { #principal_field: ::ic_cdk::export::Principal::from_slice(&__next_id()) #state_fields_test #default_fields };
                CANISTERS.with(|v| ::std::cell::RefCell::borrow_mut(v).insert(instance.principal(), instance.clone()));

                instance
            }

            #[cfg(target_arch = "wasm32")]
            fn from_principal(principal: ::ic_cdk::export::Principal) -> Self {
                Self { #principal_field: principal #state_fields_wasm #default_fields }
            }

            #[cfg(not(target_arch = "wasm32"))]
            fn from_principal(principal: ::ic_cdk::export::Principal) -> Self {
                let registry: ::std::rc::Rc<::std::cell::RefCell<::std::collections::HashMap<::ic_cdk::export::Principal, #name>>>  = CANISTERS.with(|v| v.clone());
                let mut registry = ::std::cell::RefCell::borrow_mut(&registry);
                registry.get(&principal).expect(&format!("Canister of type {} with principal {} is not registered.", ::std::any::type_name::<Self>(), principal)).clone()
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
    stable_fields: &[(proc_macro2::Ident, &Type)],
) -> proc_macro2::TokenStream {
    if stable_fields.is_empty() {
        return quote! {};
    }

    let state_gets = stable_fields.iter().map(|(name, field_type)| {
        quote! {
            let #name = #field_type::get();
        }
    });

    let state_borrows = stable_fields.iter().map(|(name, _)| {
        quote! {
            &* #name.borrow(),
        }
    });

    let field_names = stable_fields.iter().map(|(name, _)| name.clone());

    let fields_assignment = stable_fields.iter().map(|(name, field_type)| {
        quote! {
            #field_type::get().replace(#name);
        }
    });

    quote! {
        impl #struct_name {
            #[cfg(all(target_arch = "wasm32", feature = "export_api"))]
            #[export_name = "canister_pre_upgrade"]
            fn __pre_upgrade() {
                use ::ic_storage::IcStorage;

                #(#state_gets)*

                ::ic_cdk::storage::stable_save((
                    #( #state_borrows)*
                ))
                .unwrap();
            }

            #[cfg(all(target_arch = "wasm32", feature = "export_api"))]
            #[export_name = "canister_post_upgrade"]
            fn __post_upgrade() {
                use ::ic_storage::IcStorage;

                let (#( #field_names,)*) = ::ic_cdk::storage::stable_restore().unwrap();

                #( #fields_assignment )*
            }
        }
    }
}

fn is_state_field_stable(_field: &Field) -> bool {
    // todo
    true
}

fn is_principal_attr(attribute: &Attribute) -> bool {
    attribute.path.is_ident("id")
}

fn is_state_attr(attribute: &Attribute) -> bool {
    attribute.path.is_ident("state")
}

fn get_state_type(input_type: &Type) -> &Type {
    let ref_cell = extract_generic("Rc", input_type, input_type);
    extract_generic("RefCell", ref_cell, input_type)
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
    panic!("State field type must be Rc<RefCell<T>> where T: IcStorage, but the actual type is {input_type:?}")
}
