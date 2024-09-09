use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use serde::Deserialize;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{parse_macro_input, ItemFn, ReturnType};

#[derive(Default, Deserialize, Debug)]
struct ExportCandidAttr {}

impl Parse for ExportCandidAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            Ok(Self {})
        } else {
            Err(syn::Error::new(
                input.span(),
                "unexpected attribute argument",
            ))
        }
    }
}

pub(crate) fn export_candid(attr: TokenStream, input: TokenStream) -> TokenStream {
    let _ = parse_macro_input!(attr as ExportCandidAttr);

    let input_fn = parse_macro_input!(input as ItemFn);
    let input_fn_name = input_fn.sig.ident.clone();

    match input_fn.sig.output {
        ReturnType::Default => {
            return syn::Error::new(
                input_fn.sig.span(),
                "`#[export_candid]` function must return `String`",
            )
            .to_compile_error()
            .into();
        }
        ReturnType::Type(_, ref ty) => {
            if ty.to_token_stream().to_string() != "String" {
                return syn::Error::new(
                    ty.span(),
                    "`#[export_candid]` function must return `String`",
                )
                .to_compile_error()
                .into();
            }
        }
    };

    let result = quote! {
        #input_fn

        #[no_mangle]
        pub fn get_candid_pointer() -> *mut std::os::raw::c_char {
            let candid_string: String = #input_fn_name();
            let c_string = std::ffi::CString::new(candid_string).unwrap();
            c_string.into_raw()
        }
    };

    result.into()
}
