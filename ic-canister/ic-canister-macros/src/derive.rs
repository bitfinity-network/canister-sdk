use proc_macro::TokenStream;
use quote::quote;
use syn::{Attribute, Data, Fields, GenericArgument, parse_macro_input, PathArguments, Type, DeriveInput};

pub fn derive_canister(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

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

    let principal_field = principal_fields[0].ident.clone().unwrap();

    let state_fields = state_fields.iter().map(|x| {
        let field_name = x.ident.clone().unwrap();
        let field_type = get_state_type(&x.ty);
        (
            quote! {#field_name : <#field_type as ::ic_storage::IcStorage>::get()},
            quote! {#field_name : ::std::rc::Rc::new(::std::cell::RefCell::new(<#field_type as ::std::default::Default>::default()))}
        )
    });
    let (state_fields_wasm, state_fields_test) = if state_fields.len() > 0 {
        let mut state_fields_wasm = vec![];
        let mut state_fields_test = vec![];
        for (field_wasm, field_test) in state_fields {
            state_fields_wasm.push(field_wasm);
            state_fields_test.push(field_test);
        }
        (
            quote! {, #(#state_fields_wasm),* },
            quote! {, #(#state_fields_test),* },
        )
    } else {
        (
            quote! {},
            quote! {},
        )
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

    let expanded = quote! {
        #[cfg(not(target_arch = "wasm32"))]
        thread_local! {
            static CANISTERS: ::std::rc::Rc<::std::cell::RefCell<::std::collections::HashMap<Principal, #name>>> = ::std::rc::Rc::new(::std::cell::RefCell::new(::std::collections::HashMap::new()));
            static __NEXT_ID: ::std::sync::atomic::AtomicU64 = 5.into();
        }

        #[cfg(not(target_arch = "wasm32"))]
        fn __next_id() -> [u8; 8] {
            __NEXT_ID.with(|v| v.fetch_add(1, ::std::sync::atomic::Ordering::SeqCst).to_le_bytes())
        }

        impl ::ic_canister::Canister for #name {
            #[cfg(target_arch = "wasm32")]
            fn init_instance() -> Self {
                Self { #principal_field : ::ic_cdk::export::Principal::anonymous() #state_fields_wasm #default_fields }
            }

            #[cfg(not(target_arch = "wasm32"))]
            fn init_instance() -> Self {
                let instance = Self { #principal_field: ::ic_cdk::export::Principal::from_slice(&__next_id()) #state_fields_test #default_fields };
                CANISTERS.with(|v| ::std::cell::RefCell::borrow_mut(v).insert(instance.principal, instance.clone()));

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
    };

    TokenStream::from(expanded)
}

fn is_principal_attr(attribute: &Attribute) -> bool {
    attribute.path.is_ident("id")
}

fn is_state_attr(attribute: &Attribute) -> bool {
    attribute.path.is_ident("state")
}

fn get_state_type(input_type: &Type) -> &Type {
    let ref_cell = extract_generic("Rc", input_type, input_type);
    extract_generic("RefCell", &ref_cell, input_type)
}

fn extract_generic<'a>(type_name: &str, generic_base: &'a Type, input_type: &'a Type) -> &'a Type {
    match generic_base {
        Type::Path(v) => {
            if v.path.segments.is_empty() {
                state_type_error(input_type);
            }

            let last_segment = v.path.segments.iter().last().unwrap();
            if last_segment.ident != type_name {
                state_type_error(input_type);
            }

            match &last_segment.arguments {
                PathArguments::AngleBracketed(arg) => {
                    let args = &arg.args;
                    if args.len() != 1 {
                        state_type_error(input_type);
                    }

                    match &args[0] {
                        GenericArgument::Type(t) => t,
                        _ => state_type_error(input_type),
                    }
                }
                _ => state_type_error(input_type),
            }
        }
        _ => state_type_error(input_type),
    }
}

fn state_type_error(input_type: &Type) -> ! {
    panic!("State field type must be Rc<RefCell<T>> where T: IcStorage, but the actual type is {input_type:?}")
}
