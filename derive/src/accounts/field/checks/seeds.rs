//! Builds the `let` bindings for the elements of a `seeds = [...]` array. Used
//! by the PDA check in `checks` and by the signer-seeds code in `init_cpi`,
//! which must derive byte-identical seed material for the same field.

use {proc_macro2::TokenStream, quote::quote};

/// The "real" trailing operation on a seed expression, once any wrapping
/// `.as_ref()`/`.as_bytes()` call has been peeled off.
enum SeedTrailingOp {
    /// A `.address()` call — routed through `ToAddress::address`, which has
    /// an identity impl on `Address` (`core/src/wrappers/mod.rs`), so
    /// this is safe whether `.address()` is already present or not.
    Address,
    /// Anything else (a `.to_le_bytes()`/`.to_be_bytes()` call, a bare
    /// value, a literal, ...) — used as-is via `AsRefByteSlice`.
    Other,
}

/// Peels exactly one outer `.as_ref()`/`.as_bytes()` layer off `expr` if
/// present — these are always redundant here, since the raw value already
/// satisfies `AsRefByteSlice` directly — and classifies what remains.
/// Detection looks through a single leading `&` (e.g. `&seed.to_le_bytes()`)
/// without removing it; the caller always wraps the result in `&` again
/// regardless, so a leading reference already in the source is preserved as
/// part of what gets re-referenced, not specially unwrapped.
fn peel_seed_wrapper(expr: &syn::Expr) -> (syn::Expr, SeedTrailingOp) {
    let peeled = match expr {
        syn::Expr::MethodCall(mc)
            if mc.args.is_empty() && (mc.method == "as_ref" || mc.method == "as_bytes") =>
        {
            (*mc.receiver).clone()
        }
        _ => expr.clone(),
    };

    let inspect = match &peeled {
        syn::Expr::Reference(r) => &*r.expr,
        other => other,
    };
    let op = match inspect {
        syn::Expr::MethodCall(mc) if mc.args.is_empty() && mc.method == "address" => {
            SeedTrailingOp::Address
        }
        _ => SeedTrailingOp::Other,
    };
    (peeled, op)
}

/// Generates the `let #ident = ...;` binding for one element of a
/// `seeds = [...]` array. The PDA check and the `init` signer seeds both use
/// it, as they must derive byte-identical seed material for the same field.
///
/// A seed naming another field of the struct refers to that field's loaded
/// local; `load_order` loads every such field first, whatever the
/// declaration order. The expression is peeled of a redundant
/// `.as_ref()`/`.as_bytes()` wrapper, then either routed through
/// `ToAddress::address` (if the trailing operation is `.address()`) or used
/// directly via `AsRefByteSlice`.
pub(crate) fn seed_binding_tokens(expr: &syn::Expr, ident: &syn::Ident) -> TokenStream {
    let (peeled, op) = peel_seed_wrapper(expr);
    match op {
        SeedTrailingOp::Address => quote! {
            let #ident = naclac_lang::prelude::ToAddress::address(&(#peeled));
        },
        SeedTrailingOp::Other => quote! {
            let #ident = &#peeled;
        },
    }
}
