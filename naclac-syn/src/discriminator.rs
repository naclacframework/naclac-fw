//! Shared, single-source-of-truth helpers for values that must match
//! byte-for-byte between the code a macro actually compiles into a program
//! and any other Naclac tool (the AST-based IDL walker, the IDL's own
//! runtime) that separately needs to know the same value. Previously each
//! consumer re-implemented these independently; kept here instead so there
//! is exactly one implementation to get right.

use sha2::{Digest, Sha256};

/// Computes the 8-byte discriminator for a named item: `sha256("<prefix>:<name>")[0..8]`.
/// - Instructions: `prefix = "global"`
/// - Accounts:     `prefix = "account"`
/// - Events:       `prefix = "event"`
pub fn compute_discriminator(prefix: &str, name: &str) -> [u8; 8] {
    let preimage = format!("{prefix}:{name}");
    let hash = Sha256::digest(preimage.as_bytes());
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

/// Computes every variant's real, fully-resolved discriminant value,
/// mirroring Rust's own enum discriminant rule exactly: starts at -1; an
/// explicit `= N` resets the running value to `N`; otherwise the running
/// value increments by 1 from the previous variant. Getting this wrong for
/// an enum with an explicit discriminant would silently validate the wrong
/// tag byte on-chain, or report the wrong value in the IDL.
pub fn variant_discriminants<'a>(
    variants: impl IntoIterator<Item = &'a syn::Variant>,
) -> Vec<i128> {
    let mut last_value: i128 = -1;
    variants
        .into_iter()
        .map(|v| {
            if let Some((_, expr)) = &v.discriminant {
                last_value = parse_discriminant_expr(expr);
            } else {
                last_value = last_value.wrapping_add(1);
            }
            last_value
        })
        .collect()
}

/// Detects an enum's explicit `#[repr(uN)]`/`#[repr(iN)]` discriminant type,
/// if written — deliberately ignores a non-integer repr modifier like
/// `#[repr(C)]`/`#[repr(packed)]`/`#[repr(align(N))]` written alone, since
/// none of those alone commits to any particular discriminant width. Callers
/// that need a concrete tag type (e.g. `#[defined_type]`'s zero-copy enum
/// branch, which builds a `bytemuck::CheckedBitPattern` layout sized to
/// whatever this returns) must apply the same `u8` default on `None` that
/// they use for deciding whether to add their own `#[repr(u8)]` — treating
/// this as "no repr at all" when the enum actually has e.g. bare `#[repr(C)]`
/// would silently build a `Bits` layout narrower than the real compiled
/// enum's size (a bare `#[repr(C)]` data-carrying enum's discriminant is
/// platform-defined, not 1 byte).
pub fn detect_enum_repr(attrs: &[syn::Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| {
        if !attr.path().is_ident("repr") {
            return None;
        }
        let mut found = None;
        let _ = attr.parse_nested_meta(|meta| {
            for candidate in [
                "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128",
            ] {
                if meta.path.is_ident(candidate) {
                    found = Some(candidate.to_string());
                }
            }
            Ok(())
        });
        found
    })
}

fn parse_discriminant_expr(expr: &syn::Expr) -> i128 {
    match expr {
        syn::Expr::Unary(syn::ExprUnary {
            op: syn::UnOp::Neg(_),
            expr,
            ..
        }) => -parse_discriminant_expr(expr),
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(int),
            ..
        }) => int
            .base10_parse()
            .expect("naclac-syn: enum discriminant literal must fit in i128"),
        _ => panic!("naclac-syn: enum discriminant must be a literal integer expression"),
    }
}
