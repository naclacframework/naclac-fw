//! `#[constant]`: marks a `const` for the IDL. Without `idl-build` the item
//! is returned untouched; with it, the constant is also registered as an
//! `IdlFragment::Constant`.

use proc_macro::TokenStream;

use crate::common::diagnostics::reject_nonempty_attr;

/// Registers one `#[constant]` const as an `IdlFragment::Constant`, which the
/// `idl-build` binary collects through `naclac_idl::registry::collect`.
#[cfg(feature = "idl-build")]
fn constant_idl_build_impl(item: &syn::ItemConst) -> proc_macro2::TokenStream {
    let ident = &item.ident;
    let name = ident.to_string();
    let docs = naclac_syn::parser::extract_docs(&item.attrs);
    let item_ty = &*item.ty;
    quote::quote! {
        #[cfg(feature = "idl-build")]
        const _: () = {
            fn build() -> naclac_lang::naclac_idl::IdlConstant {
                naclac_lang::naclac_idl::idl_type::constant_of::<#item_ty>(
                    #name,
                    vec![#(#docs.into()),*],
                    &#ident,
                )
            }
            naclac_lang::naclac_idl::__private::inventory::submit! {
                naclac_lang::naclac_idl::registry::IdlFragment::Constant {
                    name: #name,
                    build,
                }
            }
        };
    }
}

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Some(err) = reject_nonempty_attr(&attr, "constant") {
        return err;
    }
    #[cfg(feature = "idl-build")]
    {
        let input = syn::parse_macro_input!(item as syn::ItemConst);
        let print_fn = constant_idl_build_impl(&input);
        quote::quote! {
            #input
            #print_fn
        }
        .into()
    }
    #[cfg(not(feature = "idl-build"))]
    {
        item
    }
}
