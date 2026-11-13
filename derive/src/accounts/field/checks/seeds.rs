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

/// A typed `seeds = Type::seeds(args)` call, as the `let` statements that bind
/// each argument to a local named from `prefix`, and the call itself, which
/// passes references to those locals: `Type::seeds` takes references so its
/// seed set borrows the values instead of copying them. An argument that ends
/// in `.address()` is routed through `ToAddress::address`, as a list seed is,
/// so it reads the same on both runtimes. The bindings must stay in scope for
/// as long as the seed set is used.
pub(crate) fn typed_seeds_call(
    call: &syn::ExprCall,
    prefix: &syn::Ident,
) -> (TokenStream, TokenStream) {
    let func = &call.func;
    let mut bindings = Vec::new();
    let mut refs = Vec::new();
    for (i, arg) in call.args.iter().enumerate() {
        let local = quote::format_ident!("{}_arg{}", prefix, i);
        let (peeled, op) = peel_seed_wrapper(arg);
        let value = match op {
            SeedTrailingOp::Address => {
                quote! { naclac_lang::prelude::ToAddress::address(&(#peeled)) }
            }
            SeedTrailingOp::Other => quote! { #arg },
        };
        bindings.push(quote! { let #local = #value; });
        refs.push(quote! { &#local });
    }
    (quote! { #(#bindings)* }, quote! { #func(#(#refs),*) })
}

/// The `let` statements that build `seeds`, and an expression of type
/// `&[&[u8]]` over its seed slices (without the bump). `tag` and the field's
/// name make the locals unique, so one function can hold several PDAs.
pub(crate) fn seed_slice_tokens(
    seeds: &crate::accounts::field::parser::PdaSeeds,
    field_ident: &syn::Ident,
    tag: &str,
) -> (TokenStream, TokenStream) {
    use crate::accounts::field::parser::PdaSeeds;
    match seeds {
        PdaSeeds::List(array) => {
            let bindings: Vec<TokenStream> = array
                .elems
                .iter()
                .enumerate()
                .map(|(i, expr)| {
                    let ident = quote::format_ident!("{}_{}_{}", tag, field_ident, i);
                    seed_binding_tokens(expr, &ident)
                })
                .collect();
            let refs: Vec<TokenStream> = (0..array.elems.len())
                .map(|i| {
                    let ident = quote::format_ident!("{}_{}_{}", tag, field_ident, i);
                    quote! { naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(&#ident) }
                })
                .collect();
            (quote! { #(#bindings)* }, quote! { &[ #( #refs, )* ] })
        }
        PdaSeeds::Typed(call) => {
            let set = quote::format_ident!("{}_{}_set", tag, field_ident);
            let slices = quote::format_ident!("{}_{}_slices", tag, field_ident);
            let (arg_bindings, call) =
                typed_seeds_call(call, &quote::format_ident!("{}_{}", tag, field_ident));
            (
                quote! {
                    #arg_bindings
                    let #set = #call;
                    let #slices = #set.as_slices();
                },
                quote! { &#slices },
            )
        }
    }
}
