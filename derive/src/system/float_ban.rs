//! Phase 1a: rejects `f32`/`f64` types and float literals. A type written as
//! `f32`/`f64` is rejected here; every other named type is collected so the
//! compiler can reject a float behind another name, such as an alias.

use {
    proc_macro2::TokenStream,
    quote::quote,
    std::collections::HashSet,
    syn::{visit::Visit, Error, Lit, PathArguments, Type},
};

/// Names of builtin types that are never floats, skipped to keep the
/// generated assertions short.
const NON_FLOAT_PRIMITIVES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "bool",
    "char", "str",
];

pub(super) struct FloatBanVisitor {
    pub(super) errors: Vec<Error>,
    /// The function's own generic parameters, which a nested `const` cannot
    /// name.
    pub(super) generics: HashSet<String>,
    /// Named types the compiler must check for being a float.
    pub(super) probed: Vec<Type>,
}

impl FloatBanVisitor {
    /// One `const` assertion per collected type, failing when the type is a
    /// float under another name.
    pub(super) fn probe_items(&self) -> Vec<syn::Stmt> {
        let message = "Naclac Error: this type is `f32`/`f64`, which isn't allowed in #[system] \
                       (non-deterministic on Solana); use integers with a fixed decimal scale.";
        self.probed
            .iter()
            .map(|ty| {
                let tokens: TokenStream = quote! {
                    const _: () = {
                        use naclac_lang::prelude::NotFloat as _;
                        assert!(!naclac_lang::prelude::FloatProbe::<#ty>::IS_FLOAT, #message);
                    };
                };
                syn::parse_quote!(#tokens)
            })
            .collect()
    }

    /// Whether a nested `const` can name `type_path` and the type might be a
    /// float: no generic arguments, no reference to the function's generics
    /// or `Self`, and not a builtin non-float.
    fn needs_probe(&self, type_path: &syn::TypePath) -> bool {
        if type_path.qself.is_some() {
            return false;
        }
        let segments = &type_path.path.segments;
        if segments
            .iter()
            .any(|segment| !matches!(segment.arguments, PathArguments::None))
        {
            return false;
        }
        if segments.iter().any(|segment| {
            segment.ident == "Self" || self.generics.contains(&segment.ident.to_string())
        }) {
            return false;
        }
        !(segments.len() == 1
            && NON_FLOAT_PRIMITIVES.contains(&segments[0].ident.to_string().as_str()))
    }
}

impl<'ast> Visit<'ast> for FloatBanVisitor {
    fn visit_type(&mut self, ty: &'ast Type) {
        if let Type::Path(type_path) = ty {
            if let Some(seg) = type_path.path.segments.last() {
                let name = seg.ident.to_string();
                if name == "f32" || name == "f64" {
                    self.errors.push(Error::new_spanned(
                        ty,
                        format!(
                            "Naclac Error: floating-point type `{name}` isn't allowed in \
                             #[system] (non-deterministic on Solana); use integers with a fixed \
                             decimal scale."
                        ),
                    ));
                } else if self.needs_probe(type_path) {
                    self.probed.push(ty.clone());
                }
            }
        }
        syn::visit::visit_type(self, ty);
    }

    fn visit_lit(&mut self, lit: &'ast Lit) {
        if let Lit::Float(f) = lit {
            self.errors.push(Error::new_spanned(
                f,
                "Naclac Error: floating-point literals aren't allowed in #[system] \
                 (non-deterministic on Solana); use integers with a fixed decimal scale, e.g. \
                 0.5 as 5_000u64 at scale 4.",
            ));
        }
        syn::visit::visit_lit(self, lit);
    }
}

#[cfg(test)]
mod tests {
    use {super::FloatBanVisitor, quote::ToTokens, std::collections::HashSet, syn::visit::Visit};

    fn visit(source: &str, generics: &[&str]) -> FloatBanVisitor {
        let function: syn::ItemFn = syn::parse_str(source).unwrap();
        let mut visitor = FloatBanVisitor {
            errors: Vec::new(),
            generics: generics
                .iter()
                .map(|g| g.to_string())
                .collect::<HashSet<_>>(),
            probed: Vec::new(),
        };
        visitor.visit_item_fn(&function);
        visitor
    }

    fn probed(visitor: &FloatBanVisitor) -> Vec<String> {
        visitor
            .probed
            .iter()
            .map(|ty| ty.to_token_stream().to_string().replace(' ', ""))
            .collect()
    }

    #[test]
    fn a_named_float_is_an_error_and_other_names_are_probed() {
        let visitor = visit(
            "fn f(a: f64, b: Price, c: Vec<Amount>, d: u64) -> Result<crate::Out> { Ok(x) }",
            &[],
        );
        assert!(visitor.errors.len() == 1);
        let probed = probed(&visitor);
        assert!(probed.contains(&"Price".to_string()), "{probed:?}");
        assert!(probed.contains(&"Amount".to_string()), "{probed:?}");
        assert!(probed.contains(&"crate::Out".to_string()), "{probed:?}");
        assert!(!probed.contains(&"u64".to_string()), "{probed:?}");
        assert!(!probed.iter().any(|ty| ty.contains('<')), "{probed:?}");
    }

    #[test]
    fn the_functions_generics_and_self_are_not_probed() {
        let visitor = visit(
            "fn f<T>(a: T, b: Self, c: T::Out) -> Result<()> { Ok(()) }",
            &["T"],
        );
        assert!(probed(&visitor).is_empty(), "{:?}", probed(&visitor));
    }
}
