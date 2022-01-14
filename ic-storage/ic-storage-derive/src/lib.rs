extern crate proc_macro;

use proc_macro::TokenStream;
use syn::DeriveInput;

#[proc_macro_derive(IcStorage)]
pub fn derive_ic_storage(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, .. } = syn::parse_macro_input!(input);
    let output = quote::quote! {
        impl ::ic_storage::IcStorage for #ident {
            fn get() -> ::std::rc::Rc<::std::cell::RefCell<Self>> {
                use ::std::rc::Rc;
                use ::std::cell::RefCell;

                thread_local! {
                    static store: Rc<RefCell<#ident>> = Rc::new(RefCell::new(#ident::default()));
                }

                store.with(|v| v.clone())
            }
        }
    };

    output.into()
}