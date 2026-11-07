//! Diagnostics shared by the attribute macros: rejecting an argument on an
//! attribute that takes none, and the silenceable heap-collection warning.

use proc_macro::TokenStream;

/// For attribute macros that take no argument at all — returns a
/// `compile_error!` if `attr` is non-empty, `None` otherwise.
pub(crate) fn reject_nonempty_attr(attr: &TokenStream, macro_name: &str) -> Option<TokenStream> {
    if attr.is_empty() {
        return None;
    }
    Some(
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("Naclac Error: #[{macro_name}] does not take any arguments."),
        )
        .to_compile_error()
        .into(),
    )
}

/// Tokens that make the compiler print `note` as a warning, valid wherever an
/// item or a statement is. Stable proc macros cannot emit a warning directly,
/// so it is produced by calling a `#[deprecated]` function.
pub(crate) fn compiler_warning(note: &str) -> proc_macro2::TokenStream {
    quote::quote! {
        const _: () = {
            #[deprecated(note = #note)]
            const fn __naclac_warning() {}
            __naclac_warning();
        };
    }
}

/// A warning for a heap-allocated `Vec<T>`/`String` instruction argument in a
/// zero-copy program, suggesting `suggested_ty` instead.
///
/// The caller silences it per argument with `#[allow_heap]`, which the caller
/// strips and checks before calling this function; it is deliberately not
/// `#[allow(deprecated)]`, which would hide unrelated deprecation warnings.
pub(crate) fn heap_collection_warning(
    arg_name: &str,
    found_ty: &str,
    suggested_ty: &str,
) -> proc_macro2::TokenStream {
    compiler_warning(&format!(
        "Naclac: argument `{arg_name}` is `{found_ty}` in a zero-copy program — this heap-allocates \
         and copies on every deserialize. Consider `{suggested_ty}` for zero-copy (no-allocation) \
         parsing instead, or add `#[allow_heap]` to this argument to silence this warning."
    ))
}
