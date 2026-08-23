//! Structural (AST-based) classification of field/argument types, shared
//! across every macro in this crate that needs to answer "what kind of type
//! is this" — replaces the crate's former pattern of independently
//! stringifying a `syn::Type` and substring-matching the result, which
//! misclassifies any type whose name merely contains another type's name
//! (e.g. `AccountInfoWrapper<T>` matching `"Account"`), can't distinguish a
//! trailing vs. buried occurrence, and isn't validated by the type system
//! the way matching `Type::Path`'s real variants and segments is.

use syn::{GenericArgument, PathArguments, Type};

/// The last path segment's ident, after transparently unwrapping any number
/// of `Box<...>` layers (a boxed field/arg is the same underlying kind as
/// its unboxed form). `None` for anything that isn't a path type at all
/// (references, slices, tuples, ...).
pub(crate) fn base_ident(ty: &Type) -> Option<syn::Ident> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident == "Box" {
        if let PathArguments::AngleBracketed(args) = &segment.arguments {
            for arg in &args.args {
                if let GenericArgument::Type(inner) = arg {
                    return base_ident(inner);
                }
            }
        }
    }
    Some(segment.ident.clone())
}

/// True if `ty`'s base ident (after `Box<...>` unwrapping) is exactly `name`.
pub(crate) fn is_exactly(ty: &Type, name: &str) -> bool {
    base_ident(ty).is_some_and(|id| id == name)
}

/// The first generic type argument of `ty`'s last path segment — e.g. the
/// `T` in `Vec<T>`/`Account<T>`/`Program<T>` — if there is one. Does not
/// unwrap `Box<...>` first; callers that need the inner type of a boxed
/// wrapper should call `base_ident`/resolve the box separately.
pub(crate) fn first_generic_type(ty: &Type) -> Option<&Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    args.args.iter().find_map(|a| match a {
        GenericArgument::Type(t) => Some(t),
        _ => None,
    })
}

/// True if `ty`'s first generic argument's base ident is exactly `name` —
/// e.g. `inner_is(Account<TokenAccount>, "TokenAccount")`,
/// `inner_is(Program<System>, "System")`.
pub(crate) fn inner_is(ty: &Type, name: &str) -> bool {
    first_generic_type(ty).is_some_and(|inner| is_exactly(inner, name))
}

/// True if `ty`'s outermost segment is exactly `Option` — deliberately not
/// `Box<...>`-transparent (unlike `is_exactly`/`base_ident`) in the *outer*
/// direction: `Box<Option<T>>` (Box wrapping Option) is not detected here
/// and is out of scope, since every `Some`/`None` match/`.as_mut()` call
/// site across the macro crate would need an extra explicit deref to see
/// through the outer `Box` (Rust doesn't match through `Box` implicitly the
/// way it does through `&`).
///
/// `Option<Box<T>>` (Box *inside* Option — the standard Rust idiom for an
/// optional heap-allocated value, and null-pointer-optimized) is a
/// different story and needs no special handling at all: this function only
/// inspects the outermost segment, so it's already detected as optional,
/// and `option_inner_type` below already returns the inner `Box<T>` as-is.
/// Every downstream call site (loader, `to_account_metas`/`to_account_infos`,
/// `close`/`realloc`, etc.) then works transparently through naclac-core's
/// existing blanket `NaclacAccount`/`ToAddress`/`ToAccountInfo` impls for
/// `Box<T>` — the same impls a plain (non-optional) `Box<Account<T>>>` field
/// already relies on. Confirmed working via
/// `tests/optional-accounts/programs/optional_accounts_{borsh,zc}`'s
/// `touch_boxed_optional` instruction and tests. Non-pinocchio only: those
/// `Box<T>` blanket impls don't exist on pinocchio at all (pinocchio's
/// `Account<T>` is already pointer-sized, so boxing it buys nothing there —
/// mirrors the existing plain `Box<Account<T>>` case, also non-pinocchio-only,
/// in `tests/stack-safety/`).
pub(crate) fn is_option_account(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    type_path
        .path
        .segments
        .last()
        .is_some_and(|s| s.ident == "Option")
}

/// The inner `T` of `Option<T>`, if `ty`'s outermost segment is `Option`.
/// `None` for a bare (non-optional) field. If `T` is itself `Box<U>` (i.e.
/// `ty` is `Option<Box<U>>`), this returns `Box<U>` unchanged — see
/// `is_option_account`'s doc comment for why that's already correct as-is.
pub(crate) fn option_inner_type(ty: &Type) -> Option<&Type> {
    if is_option_account(ty) {
        first_generic_type(ty)
    } else {
        None
    }
}

/// True if `ty` is the deprecated `Pubkey` type (replaced by `Address`).
pub(crate) fn is_deprecated_pubkey(ty: &Type) -> bool {
    is_exactly(ty, "Pubkey")
}

/// True if `ty` is `&[u8]` specifically — a byte-slice reference, routed
/// through the raw remaining-bytes decode path instead of length-prefixed.
pub(crate) fn is_byte_slice_ref(ty: &Type) -> bool {
    if let Type::Reference(r) = ty {
        if let Type::Slice(slice) = &*r.elem {
            return is_exactly(&slice.elem, "u8");
        }
    }
    false
}

/// How a `Vec`/`String`-shaped type should be handled: an explicit
/// zero-copy opt-in (`ZcVec<T>`/`Span<T>`/`ZcString`, no allocation) or a
/// heap-allocated `Vec<T>`/`String`, or neither (a fixed-size/Pod type).
pub(crate) enum DynamicKind {
    /// `ZcVec<T>` or `Span<T>`.
    ZcVec,
    /// `ZcString`.
    ZcString,
    /// `Vec<T>`.
    HeapVec,
    /// `String`.
    HeapString,
    /// Anything else.
    Fixed,
}

pub(crate) fn classify_dynamic(ty: &Type) -> DynamicKind {
    match base_ident(ty)
        .as_ref()
        .map(syn::Ident::to_string)
        .as_deref()
    {
        Some("ZcVec") | Some("Span") => DynamicKind::ZcVec,
        Some("ZcString") => DynamicKind::ZcString,
        Some("Vec") => DynamicKind::HeapVec,
        Some("String") => DynamicKind::HeapString,
        _ => DynamicKind::Fixed,
    }
}

/// True if `ty`'s base ident is one of naclac's account-wrapper names
/// (`Account<T>`, `InterfaceAccount<T>` — see `naclac-core/src/wrappers/accounts/`).
/// Both cfg-route internally between the Borsh backend and the zero-copy
/// backend the same way, so this is the single check that matters anywhere
/// the macro needs to know "is this field a real account wrapper" — the one
/// place that knows the full set of names, so adding a future wrapper is a
/// one-line change here, not a hunt across the macro crate.
pub(crate) fn is_account_wrapper(ty: &Type) -> bool {
    matches!(
        base_ident(ty)
            .as_ref()
            .map(syn::Ident::to_string)
            .as_deref(),
        Some("Account") | Some("InterfaceAccount")
    )
}
