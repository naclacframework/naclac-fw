/// The name the IDL gives a field, account or argument written as `raw` in
/// source: leading underscores (the "intentionally unused" marker) dropped.
pub fn idl_name(raw: &str) -> String {
    raw.trim_start_matches('_').to_string()
}

/// True if any `#[derive(..)]` in `attrs` lists a path whose last segment is
/// exactly `name`. A derive whose name merely contains `name` does not match.
pub fn derives(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("derive")
            && attr
                .parse_args_with(
                    syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
                )
                .is_ok_and(|paths| {
                    paths
                        .iter()
                        .any(|path| path.segments.last().is_some_and(|s| s.ident == name))
                })
    })
}

/// Extracts `///`/`#[doc = "..."]` doc-comment lines from an attribute list,
/// trimmed of leading whitespace, in source order.
pub fn extract_docs(attrs: &[syn::Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|attr| {
            if !attr.path().is_ident("doc") {
                return None;
            }
            let syn::Meta::NameValue(nv) = &attr.meta else {
                return None;
            };
            let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = &nv.value
            else {
                return None;
            };
            Some(s.value().trim().to_string())
        })
        .collect()
}
