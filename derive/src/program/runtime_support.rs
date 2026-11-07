//! Program-level support code that depends on how the crate declares
//! `no_std`: reading that declaration from the crate root and generating the
//! matching panic handler.

use quote::quote;

/// How the program crate's root file declares `no_std`. An attribute macro
/// cannot see its crate's inner attributes, so `crate_root_std` reads them
/// from the crate root file itself.
enum CrateStd {
    Std,
    NoStd,
    /// `#![cfg_attr(<predicate>, no_std)]`: `no_std` exactly when the
    /// predicate holds.
    NoStdWhen(proc_macro2::TokenStream),
}

/// Reads the program crate's root file (`[lib] path` in its `Cargo.toml`,
/// else `src/lib.rs`) and reports how it declares `no_std`. An error means
/// the file could not be found or parsed.
fn crate_root_std() -> Result<CrateStd, String> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| "CARGO_MANIFEST_DIR is not set".to_string())?;
    let root = naclac_syn::modules::crate_root(std::path::Path::new(&manifest_dir));
    let source = std::fs::read_to_string(&root)
        .map_err(|error| format!("cannot read the crate root {}: {error}", root.display()))?;
    let file = syn::parse_file(&source)
        .map_err(|error| format!("cannot parse the crate root {}: {error}", root.display()))?;

    for attr in &file.attrs {
        if !matches!(attr.style, syn::AttrStyle::Inner(_)) {
            continue;
        }
        if attr.path().is_ident("no_std") {
            return Ok(CrateStd::NoStd);
        }
        if attr.path().is_ident("cfg_attr") {
            let Ok(args) = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            ) else {
                continue;
            };
            let mut args = args.into_iter();
            let Some(predicate) = args.next() else {
                continue;
            };
            if args.any(|meta| meta.path().is_ident("no_std")) {
                return Ok(CrateStd::NoStdWhen(quote! { #predicate }));
            }
        }
    }
    Ok(CrateStd::Std)
}

/// The program's panic support for the Solana target, matched to how its
/// crate declares `no_std`: the `custom_panic` hook for a `std` crate, the
/// `no_std` choice (`__naclac_nostd_panic!`) for a `no_std` one, and both
/// behind the crate's own predicate for a `cfg_attr` one. Omitted under
/// `no-entrypoint`, where the program that links this one supplies it.
pub(super) fn panic_support_tokens() -> proc_macro2::TokenStream {
    let no_entrypoint = quote! { #[cfg(not(feature = "no-entrypoint"))] };
    match crate_root_std() {
        Ok(CrateStd::Std) => quote! {
            #no_entrypoint
            naclac_lang::__naclac_panic_hook!();
        },
        Ok(CrateStd::NoStd) => quote! {
            #no_entrypoint
            naclac_lang::__naclac_nostd_panic!();
        },
        Ok(CrateStd::NoStdWhen(predicate)) => quote! {
            #no_entrypoint
            #[cfg(#predicate)]
            naclac_lang::__naclac_nostd_panic!();
            #no_entrypoint
            #[cfg(not(#predicate))]
            naclac_lang::__naclac_panic_hook!();
        },
        Err(message) => {
            let message = format!(
                "Naclac Error: #[program] could not determine whether this crate is `no_std`: {message}"
            );
            quote! {
                #[cfg(all(target_os = "solana", not(feature = "no-entrypoint")))]
                compile_error!(#message);
            }
        }
    }
}
