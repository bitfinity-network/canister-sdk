extern crate proc_macro;

use proc_macro::TokenStream;
use syn::DeriveInput;

#[proc_macro_derive(IcStorage)]
pub fn derive_ic_storage(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, .. } = syn::parse_macro_input!(input);
    let output = quote::quote! {
        #[cfg(target_family = "wasm")]
        impl IcStorage for #ident {
            fn get() -> ::std::rc::Rc<::std::cell::RefCell<Self>> {
                use ::std::rc::Rc;
                use ::std::cell::RefCell;

                thread_local! {
                    static store: Rc<RefCell<#ident>> = Rc::new(RefCell::new(#ident::default()));
                }

                store.with(|v| v.clone())
            }
        }

        #[cfg(not(target_family = "wasm"))]
        impl IcStorage for #ident {
            fn get() -> ::std::rc::Rc<::std::cell::RefCell<Self>> {
                use ::std::rc::Rc;
                use ::std::cell::RefCell;
                use ::std::collections::HashMap;
                use ::ic_exports::candid::Principal;

                thread_local! {
                    static store: RefCell<HashMap<Principal, Rc<RefCell<#ident>>>> = RefCell::new(HashMap::default());
                }

                let id = ::ic_exports::ic_kit::ic::id();
                store.with(|v| {
                    let mut borrowed_store = v.borrow_mut();
                    (*borrowed_store.entry(id).or_default()).clone()
                })
            }
        }
    };

    output.into()
}
