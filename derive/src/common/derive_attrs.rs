//! Lookup of `#[derive(...)]` entries the user wrote on an item, shared by the
//! macros that add or reject derives of their own.

/// The first `#[derive(...)]` attribute listing a path whose last segment is
/// `name` (so `Default` matches both `Default` and `std::default::Default`).
/// A derive attribute whose arguments do not parse as a path list is skipped.
pub(crate) fn derive_attr_listing<'a>(
    attrs: &'a [syn::Attribute],
    name: &str,
) -> Option<&'a syn::Attribute> {
    attrs.iter().find(|attr| {
        if !attr.path().is_ident("derive") {
            return false;
        }
        let Ok(paths) = attr.parse_args_with(
            syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
        ) else {
            return false;
        };
        paths
            .iter()
            .any(|p| p.segments.last().is_some_and(|s| s.ident == name))
    })
}
