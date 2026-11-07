//! `#[constant]`: marks a `const` for the IDL. Without `idl-build` the item
//! is returned untouched; with it, a print function for the constant is
//! emitted next to it.

use proc_macro::TokenStream;

use crate::common::diagnostics::reject_nonempty_attr;

/// Tokens evaluating to the IDL `value` string of a constant of type `ty`
/// whose value is `expr`: an `Address` as base58 on every backend, anything
/// else as its `Debug` text.
#[cfg(feature = "idl-build")]
pub(crate) fn constant_value_tokens(
    ty: &syn::Type,
    expr: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if crate::type_classify::is_exactly(ty, "Address") {
        quote::quote! { naclac_lang::prelude::base58::encode_address_to_string(&#expr) }
    } else {
        quote::quote! { format!("{:?}", #expr) }
    }
}

/// Builds the print function for one `#[constant]` const, returning its
/// `IdlConstant`. `#[program]` calls it from the `idl-build` `main()` by name:
/// `__naclac_idl_print_const_<lowercased const name>`, which must match the
/// name `program.rs` generates its calls with.
#[cfg(feature = "idl-build")]
fn constant_idl_build_impl(item: &syn::ItemConst) -> proc_macro2::TokenStream {
    let name = item.ident.to_string();
    let expr = &item.expr;
    let docs = naclac_syn::parser::extract_docs(&item.attrs);
    let item_ty = crate::common::crate_source::crate_aliases().resolve(&item.ty);
    let ty_value = naclac_syn::parser::rust_type_to_idl(
        &item_ty,
        &crate::common::crate_source::crate_constants(),
    );
    let ty_json = ty_value.to_string();
    let fn_name = quote::format_ident!(
        "__naclac_idl_print_const_{}",
        item.ident.to_string().to_lowercase()
    );
    let value = constant_value_tokens(&item_ty, &quote::quote! { #expr });
    quote::quote! {
        #[cfg(feature = "idl-build")]
        #[doc(hidden)]
        pub fn #fn_name() -> naclac_lang::naclac_idl::IdlConstant {
            naclac_lang::naclac_idl::IdlConstant {
                name: #name.into(),
                docs: vec![#(#docs.into()),*],
                ty: naclac_lang::naclac_idl::serde_json::from_str(#ty_json)
                    .expect("naclac idl-build: generated constant type JSON must parse"),
                value: #value,
            }
        }
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
