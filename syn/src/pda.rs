//! PDA seed helpers the `#[derive(Accounts)]` macro uses at expansion time:
//! integer-literal seeds resolved to exact bytes, and the account/field path
//! of a seed expression.

use {quote::ToTokens, syn::Expr};

/// Maps an integer literal's type suffix (`u8`, `u16`, ...) to its byte
/// width. `None` for an unsuffixed or non-integer-primitive literal — the
/// callers below treat that as a hard error rather than guessing a width,
/// since a wrong guess here would silently derive the wrong PDA.
fn int_literal_byte_width(int_lit: &syn::LitInt) -> Option<usize> {
    match int_lit.suffix() {
        "u8" | "i8" => Some(1),
        "u16" | "i16" => Some(2),
        "u32" | "i32" => Some(4),
        "u64" | "i64" => Some(8),
        "u128" | "i128" => Some(16),
        _ => None,
    }
}

/// Resolves a bare integer-literal seed element (e.g. `5u8` inside `&[5u8]`)
/// to its single byte. Only a `u8`-suffixed (or unsuffixed, which infers `u8`
/// in this single-byte-slice-element context) literal is a byte on its own;
/// anything wider needs an explicit `.to_le_bytes()`/`.to_be_bytes()` call,
/// handled separately.
pub fn int_literal_seed_byte(int_lit: &syn::LitInt) -> Vec<u8> {
    let suffix = int_lit.suffix();
    if suffix.is_empty() || suffix == "u8" {
        let value: u8 = int_lit.base10_parse().unwrap_or_else(|e| {
            panic!(
                "Naclac Error: literal seed `{}` could not be parsed as a `u8`: {e}",
                int_lit.to_token_stream()
            );
        });
        return vec![value];
    }
    panic!(
        "Naclac Error: literal seed `{}` has an integer type suffix (`{suffix}`) that isn't \
         `u8` — a bare integer seed element must resolve to a single byte. Use \
         `.to_le_bytes()`/`.to_be_bytes()` (with an explicit type suffix) to seed with a wider \
         integer instead.",
        int_lit.to_token_stream()
    );
}

/// Resolves a literal integer's `.to_le_bytes()`/`.to_be_bytes()` call (e.g.
/// `0u16.to_le_bytes()`) to its byte sequence. Requires an explicit type
/// suffix on the literal so the byte width is known exactly — the same real
/// width `.to_le_bytes()`/`.to_be_bytes()` produces at runtime — rather than
/// guessed.
pub fn int_literal_seed_bytes(int_lit: &syn::LitInt, big_endian: bool) -> Vec<u8> {
    let Some(width) = int_literal_byte_width(int_lit) else {
        panic!(
            "Naclac Error: literal seed `{}.{}()` needs an explicit integer type suffix (e.g. \
             `0u16`, not a bare `0`) so its byte width can be resolved without guessing.",
            int_lit.to_token_stream(),
            if big_endian {
                "to_be_bytes"
            } else {
                "to_le_bytes"
            }
        );
    };
    let value: u128 = int_lit.base10_parse().unwrap_or_else(|e| {
        panic!(
            "Naclac Error: literal seed `{}` could not be parsed as an integer: {e}",
            int_lit.to_token_stream()
        );
    });
    truncate_le_bytes(value, width, big_endian)
}

/// Truncates `value`'s little-endian bytes to the first `width` of them,
/// then reverses to big-endian if requested — the real byte-computation
/// behind `int_literal_seed_bytes`, extracted as its own pure function so
/// it's directly provable (see docs/plan/kani-audit.md). `width` must be
/// `<= 16` (the only widths `int_literal_byte_width` ever produces, from a
/// fixed `u8`/`u16`/`u32`/`u64`/`u128` suffix match) — the real precondition
/// every caller in this file already guarantees, not an arbitrary width.
fn truncate_le_bytes(value: u128, width: usize, big_endian: bool) -> Vec<u8> {
    let mut bytes = value.to_le_bytes()[..width].to_vec();
    if big_endian {
        bytes.reverse();
    }
    bytes
}

pub fn extract_field_path(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(p) => Some(p.path.segments.last().unwrap().ident.to_string()),
        Expr::Field(f) => {
            let base_path = extract_field_path(&f.base)?;
            let field_name = f.member.to_token_stream().to_string();
            Some(format!("{}.{}", base_path, field_name))
        }
        Expr::MethodCall(m) => extract_field_path(&m.receiver),
        Expr::Try(t) => extract_field_path(&t.expr),
        Expr::Reference(r) => extract_field_path(&r.expr),
        _ => None,
    }
}

pub fn extract_root_ident(expr: &Expr) -> String {
    match expr {
        Expr::Path(p) => p.path.segments.last().unwrap().ident.to_string(),
        Expr::Field(f) => extract_root_ident(&f.base),
        Expr::MethodCall(m) => extract_root_ident(&m.receiver),
        Expr::Try(t) => extract_root_ident(&t.expr), // like `pool_account.load()?`
        _ => expr.to_token_stream().to_string().replace(" ", ""),
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `truncate_le_bytes` never panics within its real contract
    /// (`width <= 16`, the only values `int_literal_byte_width` ever
    /// produces), and correctness — a wrong byte width or endianness here
    /// would silently derive the wrong PDA for any account seeded with a
    /// multi-byte integer literal.
    #[kani::proof]
    fn prove_truncate_le_bytes_within_contract_is_correct() {
        let value: u128 = kani::any();
        let width: usize = kani::any();
        kani::assume(width <= 16);
        let big_endian: bool = kani::any();

        let bytes = truncate_le_bytes(value, width, big_endian);
        assert_eq!(bytes.len(), width);

        let le_prefix = &value.to_le_bytes()[..width];
        if big_endian {
            let expected: Vec<u8> = le_prefix.iter().rev().copied().collect();
            assert_eq!(bytes, expected);
        } else {
            assert_eq!(bytes, le_prefix);
        }
    }
}
