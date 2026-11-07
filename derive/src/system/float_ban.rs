//! Phase 1a: rejects `f32`/`f64` types and float literals.

use syn::{visit::Visit, Error, Lit, Type};

pub(super) struct FloatBanVisitor {
    pub(super) errors: Vec<Error>,
    pub(super) aliases: naclac_syn::aliases::Aliases,
}

impl<'ast> Visit<'ast> for FloatBanVisitor {
    fn visit_type(&mut self, ty: &'ast Type) {
        if let Type::Path(type_path) = &self.aliases.resolve(ty) {
            if let Some(seg) = type_path.path.segments.last() {
                let name = seg.ident.to_string();
                if name == "f32" || name == "f64" {
                    self.errors.push(Error::new_spanned(
                        ty,
                        format!(
                            "[naclac::system] Floating-point type `{name}` is not allowed.\n\
                             Floating-point arithmetic is non-deterministic across Solana/SBF \
                             validators and will produce inconsistent state.\n\
                             Use integer arithmetic with a fixed decimal scale instead."
                        ),
                    ));
                }
            }
        }
        syn::visit::visit_type(self, ty);
    }

    fn visit_lit(&mut self, lit: &'ast Lit) {
        if let Lit::Float(f) = lit {
            self.errors.push(Error::new_spanned(
                f,
                "[naclac::system] Floating-point literal is not allowed.\n\
                 Floating-point arithmetic is non-deterministic across Solana/SBF validators.\n\
                 Use an integer literal with a fixed decimal scale instead\n\
                 (e.g. `0.5` → `5_000u64` with scale 4, or use basis points).",
            ));
        }
        syn::visit::visit_lit(self, lit);
    }
}
