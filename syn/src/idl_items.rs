//! The `#[error_code]` offset rules: the default offset and the parser of the
//! macro's `offset = N` argument.

/// The on-chain code of an `#[error_code]` enum's first possible variant
/// (discriminant 0) when the enum sets no `offset`. Must equal
/// `naclac_core::error::USER_ERROR_BASE`; `#[error_code]` asserts it at
/// compile time.
pub const DEFAULT_ERROR_OFFSET: u32 = 6000;

/// The `offset` of the argument tokens of `#[error_code(..)]`: the code of
/// discriminant 0. Any other argument is an error.
pub fn error_code_offset_tokens(tokens: proc_macro2::TokenStream) -> syn::Result<u32> {
    if tokens.is_empty() {
        return Ok(DEFAULT_ERROR_OFFSET);
    }
    let nv: syn::MetaNameValue = syn::parse2(tokens)?;
    if !nv.path.is_ident("offset") {
        return Err(syn::Error::new_spanned(
            &nv.path,
            "Naclac Error: `#[error_code]` accepts only `offset = N`.",
        ));
    }
    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(int),
        ..
    }) = &nv.value
    else {
        return Err(syn::Error::new_spanned(
            &nv.value,
            "Naclac Error: `offset` must be an integer literal.",
        ));
    };
    int.base10_parse::<u32>()
}
