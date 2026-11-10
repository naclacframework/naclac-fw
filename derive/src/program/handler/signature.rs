//! A handler's signature: normalising its `Context<..>` and
//! `ZcVec`/`ZcString` parameters, and reading out its accounts struct and
//! instruction arguments.

use {
    crate::type_classify,
    std::collections::HashSet,
    syn::{FnArg, Ident, ItemFn, Pat, Type},
};

/// Rewrites the handler's parameters in place and returns the arguments marked
/// `#[allow_heap]`, which skip the heap-collection warning. The marker is
/// stripped here, as it is not an attribute rustc knows.
///
/// `ZcVec<T>`/`ZcString` are re-qualified to their prelude paths (only the
/// spelling changes; a written limit, as in `ZcVec<u64, 8>`, is kept), and a
/// `Context<T>` parameter, possibly behind a
/// reference and with lifetimes, becomes plain `Context<T>`.
pub(super) fn rewrite_signature(new_func: &mut ItemFn) -> syn::Result<HashSet<Ident>> {
    let mut heap_allowed_args: HashSet<Ident> = HashSet::new();
    for arg in &mut new_func.sig.inputs {
        if let FnArg::Typed(pat_type) = arg {
            let had_allow_heap = pat_type
                .attrs
                .iter()
                .any(|a| a.path().is_ident("allow_heap"));
            pat_type.attrs.retain(|a| !a.path().is_ident("allow_heap"));
            if had_allow_heap {
                if let Pat::Ident(pat_ident) = &*pat_type.pat {
                    heap_allowed_args.insert(pat_ident.ident.clone());
                }
            }

            // Re-qualify a bare `ZcVec<T>`/`ZcString` to its fully-qualified
            // prelude path. Always safe regardless of representation —
            // this only changes spelling, never the type the signature
            // resolves to.
            let ty = &pat_type.ty;
            if type_classify::is_exactly(ty, "ZcVec")
                && type_classify::first_generic_type(ty).is_none()
            {
                return Err(syn::Error::new_spanned(
                    ty,
                    "Naclac Error: `ZcVec` needs an element type, e.g. `ZcVec<u8>`.",
                ));
            }
            if type_classify::is_exactly(ty, "ZcVec") || type_classify::is_exactly(ty, "ZcString") {
                if let Some(view_ty) = type_classify::zc_view_type(ty) {
                    *pat_type.ty = syn::parse2(view_ty)?;
                }
            }

            let mut inner_ty = (*pat_type.ty).clone();

            if let Type::Reference(r) = &inner_ty {
                inner_ty = (*r.elem).clone();
            }

            if let Type::Path(type_path) = &mut inner_ty {
                if let Some(segment) = type_path.path.segments.last_mut() {
                    if segment.ident == "Context" {
                        if let syn::PathArguments::AngleBracketed(args) = &mut segment.arguments {
                            let mut type_arg_idx = None;
                            for (i, arg) in args.args.iter().enumerate() {
                                if let syn::GenericArgument::Type(_) = arg {
                                    type_arg_idx = Some(i);
                                    break;
                                }
                            }

                            if let Some(idx) = type_arg_idx {
                                if let syn::GenericArgument::Type(Type::Path(inner_path)) =
                                    &mut args.args[idx]
                                {
                                    if let Some(last_seg) = inner_path.path.segments.last_mut() {
                                        last_seg.arguments = syn::PathArguments::None;
                                    }
                                }

                                let mut new_args = syn::punctuated::Punctuated::<
                                    syn::GenericArgument,
                                    syn::Token![,],
                                >::new();
                                new_args.push(args.args[idx].clone());
                                args.args = new_args;
                            }
                        }

                        // Standardize the parameter type to Context<T>
                        // avoiding &mut references to prevent type mismatches in CPI wrappers.
                        *pat_type.ty = syn::parse_quote! { #inner_ty };
                    }
                }
            }
        }
    }
    Ok(heap_allowed_args)
}

/// What a handler's signature declares: its accounts struct and the
/// instruction arguments, in declaration order.
pub(super) struct HandlerSignature<'a> {
    pub(super) ctx_struct: syn::Path,
    pub(super) arg_names: Vec<&'a Ident>,
    pub(super) arg_types: Vec<&'a Type>,
}

/// Reads the accounts struct and arguments off `func`'s parameters.
pub(super) fn parse_signature(func: &ItemFn) -> syn::Result<HandlerSignature<'_>> {
    let mut context_struct_name = None;
    let mut arg_names = Vec::new();
    let mut arg_types = Vec::new();

    for (position, arg) in func.sig.inputs.iter().enumerate() {
        let FnArg::Typed(pat_type) = arg else {
            return Err(syn::Error::new_spanned(
                arg,
                "Naclac Error: an instruction handler can't take `self`.",
            ));
        };
        let Pat::Ident(pat_ident) = &*pat_type.pat else {
            return Err(syn::Error::new_spanned(
                &pat_type.pat,
                "Naclac Error: instruction arguments must be plain names, like `amount: u64`.",
            ));
        };
        if let Some(ctx_path) = extract_context_struct(&pat_type.ty) {
            if context_struct_name.is_some() {
                return Err(syn::Error::new_spanned(
                    &pat_type.ty,
                    "Naclac Error: an instruction handler takes exactly one `Context` argument.",
                ));
            }
            if position != 0 {
                return Err(syn::Error::new_spanned(
                    &pat_type.ty,
                    "Naclac Error: the `Context` argument must come first.",
                ));
            }
            context_struct_name = Some(ctx_path);
        } else {
            arg_names.push(&pat_ident.ident);
            arg_types.push(&*pat_type.ty);
        }
    }

    let Some(ctx_struct) = context_struct_name else {
        return Err(syn::Error::new_spanned(
            &func.sig.ident,
            format!(
                "Naclac Error: instruction '{}' must have a Context argument.",
                func.sig.ident
            ),
        ));
    };
    Ok(HandlerSignature {
        ctx_struct,
        arg_names,
        arg_types,
    })
}

/// A helper function to traverse the AST and extract the underlying accounts struct
/// from `Context<MyAccounts>` or `&mut Context<MyAccounts>`.
fn extract_context_struct(ty: &Type) -> Option<syn::Path> {
    // Support both `Context<T>` and `&mut Context<T>`
    let ty_path = match ty {
        Type::Path(p) => Some(p),
        Type::Reference(r) => {
            if let Type::Path(p) = &*r.elem {
                Some(p)
            } else {
                None
            }
        }
        _ => None,
    };

    if let Some(type_path) = ty_path {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Context" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    for arg in &args.args {
                        // Ignore lifetimes, find the actual Type
                        if let syn::GenericArgument::Type(Type::Path(inner_path)) = arg {
                            let mut path = inner_path.path.clone();
                            // Strip any generic arguments (like <'info>) to avoid syntax errors when used in expressions
                            if let Some(last_segment) = path.segments.last_mut() {
                                last_segment.arguments = syn::PathArguments::None;
                            }
                            return Some(path);
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{parse_signature, rewrite_signature};

    fn signature_error(source: &str) -> String {
        let func: syn::ItemFn = syn::parse_str(source).unwrap();
        match parse_signature(&func) {
            Ok(_) => panic!("the signature must be rejected"),
            Err(err) => err.to_string(),
        }
    }

    #[test]
    fn the_context_must_be_the_single_first_argument() {
        let message = signature_error("pub fn f(amount: u64, ctx: Context<A>) {}");
        assert!(message.contains("must come first"), "{message}");
        let message = signature_error("pub fn f(a: Context<A>, b: Context<B>) {}");
        assert!(message.contains("exactly one `Context`"), "{message}");
        let message = signature_error("pub fn f(amount: u64) {}");
        assert!(message.contains("must have a Context"), "{message}");
    }

    #[test]
    fn arguments_must_be_plain_names() {
        let message = signature_error("pub fn f(ctx: Context<A>, _: u64) {}");
        assert!(message.contains("plain names"), "{message}");
        let message = signature_error("pub fn f(ctx: Context<A>, (a, b): (u64, u64)) {}");
        assert!(message.contains("plain names"), "{message}");
    }

    #[test]
    fn a_written_limit_survives_the_rewrite() {
        let mut func: syn::ItemFn =
            syn::parse_str("pub fn f(ctx: Context<A>, v: ZcVec<u64, 8>, s: ZcString<32>) {}")
                .unwrap();
        rewrite_signature(&mut func).unwrap();
        let emitted = quote::quote!(#func).to_string();
        assert!(emitted.contains("ZcVec < u64 , 8 >"), "{emitted}");
        assert!(emitted.contains("ZcString < 32 >"), "{emitted}");
    }

    #[test]
    fn a_zc_vec_without_an_element_type_is_rejected() {
        let mut func: syn::ItemFn =
            syn::parse_str("pub fn f(ctx: Context<A>, v: ZcVec) {}").unwrap();
        let message = rewrite_signature(&mut func).unwrap_err().to_string();
        assert!(message.contains("needs an element type"), "{message}");
    }
}
