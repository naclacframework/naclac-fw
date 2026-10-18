//! Structural (AST-based) classification of field/argument types, shared
//! across every macro in this crate that needs to answer "what kind of type
//! is this" — replaces the crate's former pattern of independently
//! stringifying a `syn::Type` and substring-matching the result, which
//! misclassifies any type whose name merely contains another type's name
//! (e.g. `AccountInfoWrapper<T>` matching `"Account"`), can't distinguish a
//! trailing vs. buried occurrence, and isn't validated by the type system
//! the way matching `Type::Path`'s real variants and segments is.
//!
//! A proc-macro sees only syntax, so a type is recognised here by its last
//! path segment. That is only ever used to choose a code shape or a
//! diagnostic, never to decide a size or a memory layout: those come from
//! compiler-resolved traits (`SerializedBounds`, `Pod`, the generated layout
//! assertions). A name that is not recognised must therefore either fail to
//! compile or change only the generated IDL or an error message.

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

/// An error when `ident` is a name that naclac's IDL and generated clients
/// give to a framework type, so a user type with it would be read as that type.
pub(crate) fn reserved_type_name_error(ident: &syn::Ident) -> Option<syn::Error> {
    (ident == "Bool").then(|| {
        syn::Error::new_spanned(
            ident,
            "Naclac Error: `Bool` is the name of naclac's one-byte boolean and of the IDL's \
             `bool`; a type with this name would be read as that type. Rename it.",
        )
    })
}

/// True if `ty` is the deprecated `Address` type (replaced by `Address`).
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

/// How a `Vec`/`String`/`Option`-shaped type should be handled: an explicit
/// zero-copy opt-in (`ZcVec<T>`/`ZcString`, no allocation), a
/// heap-allocated `Vec<T>`/`String`, an `Option<T>` (fixed wire size, but
/// never a native in-memory `Pod` bit-pattern match — see `OptionPod`), or
/// neither (a true fixed-size/`Pod` type).
pub(crate) enum DynamicKind {
    /// `ZcVec<T>`.
    ZcVec,
    /// `ZcString`.
    ZcString,
    /// `Vec<T>`.
    HeapVec,
    /// `String`.
    HeapString,
    /// `Option<T>`. Every consumer that dispatches per-argument/per-field via
    /// `NaclacArgs`/`NaclacPod` (never a whole-struct raw byte-cast) may
    /// treat this identically to `Fixed` — `NaclacPod`'s `Option<T>` impl
    /// (`core/src/prelude.rs`) already encodes/decodes the correct
    /// 1-byte-tag-plus-payload wire format. The one place that must *not*
    /// conflate the two: whatever decides if a struct can take the
    /// whole-struct `unsafe impl Pod`/raw-`read_unaligned` fast path — an
    /// `Option<T>`'s wire format never matches its native in-memory
    /// representation, so a struct containing one is unsound to blob-cast as
    /// a whole, unlike a struct of only genuinely-`Fixed` fields.
    OptionPod,
    /// Anything else.
    Fixed,
}

pub(crate) fn classify_dynamic(ty: &Type) -> DynamicKind {
    match base_ident(ty)
        .as_ref()
        .map(syn::Ident::to_string)
        .as_deref()
    {
        Some("ZcVec") => DynamicKind::ZcVec,
        Some("ZcString") => DynamicKind::ZcString,
        Some("Vec") => DynamicKind::HeapVec,
        Some("String") => DynamicKind::HeapString,
        Some("Option") => DynamicKind::OptionPod,
        _ => DynamicKind::Fixed,
    }
}

/// An error spanned at each `ZcVec<T>`/`ZcString` field in `fields`, or `None`
/// when there is none. Those types are views into the current instruction's
/// bytes and own no bytes themselves, so no stored or emitted type (`owner`)
/// can hold one; `advice` names what to write instead.
pub(crate) fn zc_view_field_error<'a>(
    fields: impl Iterator<Item = &'a syn::Field>,
    owner: &str,
    advice: &str,
) -> Option<syn::Error> {
    let mut combined: Option<syn::Error> = None;
    for field in fields {
        let view = match classify_dynamic(&field.ty) {
            DynamicKind::ZcVec => "ZcVec<T>",
            DynamicKind::ZcString => "ZcString",
            _ => continue,
        };
        let error = syn::Error::new_spanned(
            &field.ty,
            format!(
                "Naclac Error: '{view}' cannot be a field of {owner}. It is a view (pointer + \
                 length) into the current instruction's bytes and owns no bytes of its own. {advice}"
            ),
        );
        match &mut combined {
            Some(existing) => existing.combine(error),
            None => combined = Some(error),
        }
    }
    combined
}

/// True if `ty` is, or contains anywhere in its generic arguments, arrays or
/// references, a `FixedVec<T, N>` or `FixedString<N>`.
fn contains_fixed_capacity(ty: &Type) -> bool {
    match ty {
        Type::Path(type_path) => type_path.path.segments.iter().any(|segment| {
            segment.ident == "FixedVec"
                || segment.ident == "FixedString"
                || matches!(
                    &segment.arguments,
                    PathArguments::AngleBracketed(args) if args.args.iter().any(|arg| {
                        matches!(arg, GenericArgument::Type(inner) if contains_fixed_capacity(inner))
                    })
                )
        }),
        Type::Array(array) => contains_fixed_capacity(&array.elem),
        Type::Reference(reference) => contains_fixed_capacity(&reference.elem),
        Type::Slice(slice) => contains_fixed_capacity(&slice.elem),
        _ => false,
    }
}

/// An error spanned at each field of `fields` whose type holds a
/// `FixedVec<T, N>` or `FixedString<N>`, or `None` when there is none. A
/// client lays such a field out from the capacity recorded for the owning
/// account, defined struct or instruction; `owner` is a place that has no such
/// record, and `advice` names what to write instead.
pub(crate) fn fixed_capacity_field_error<'a>(
    fields: impl Iterator<Item = &'a syn::Field>,
    owner: &str,
    advice: &str,
) -> Option<syn::Error> {
    let mut combined: Option<syn::Error> = None;
    for field in fields {
        if !contains_fixed_capacity(&field.ty) {
            continue;
        }
        let error = syn::Error::new_spanned(
            &field.ty,
            format!(
                "Naclac Error: 'FixedVec'/'FixedString' cannot be a field of {owner}. Generated \
                 clients know a fixed-capacity field's capacity only for account, defined struct \
                 and instruction argument fields. {advice}"
            ),
        );
        match &mut combined {
            Some(existing) => existing.combine(error),
            None => combined = Some(error),
        }
    }
    combined
}

/// Gives the struct behind `input` a lifetime parameter and writes it into every
/// direct `ZcVec<T>`/`ZcString` field, since Rust requires a struct field to
/// spell out the lifetime of a borrowing type. Returns that lifetime, or
/// `None` when no field needs one, in which case `input` is left untouched.
/// A lifetime the struct already declares is reused, and a field that already
/// names a lifetime keeps it.
pub(crate) fn thread_view_lifetime(input: &mut syn::DeriveInput) -> Option<syn::Lifetime> {
    let syn::Data::Struct(data) = &mut input.data else {
        return None;
    };
    let syn::Fields::Named(fields) = &mut data.fields else {
        return None;
    };
    let is_view = |ty: &Type| {
        matches!(
            classify_dynamic(ty),
            DynamicKind::ZcVec | DynamicKind::ZcString
        )
    };
    if !fields.named.iter().any(|f| is_view(&f.ty)) {
        return None;
    }

    let lifetime = match input.generics.lifetimes().next() {
        Some(param) => param.lifetime.clone(),
        None => {
            let lifetime = syn::Lifetime::new("'a", proc_macro2::Span::call_site());
            input.generics.params.insert(
                0,
                syn::GenericParam::Lifetime(syn::LifetimeParam::new(lifetime.clone())),
            );
            lifetime
        }
    };

    for field in fields.named.iter_mut() {
        let Type::Path(type_path) = &mut field.ty else {
            continue;
        };
        let Some(segment) = type_path.path.segments.last_mut() else {
            continue;
        };
        let is_string = segment.ident == "ZcString";
        if !is_string && segment.ident != "ZcVec" {
            continue;
        }
        if matches!(segment.arguments, PathArguments::None) {
            if is_string {
                segment.arguments = PathArguments::AngleBracketed(syn::parse_quote!(<#lifetime>));
            }
            continue;
        }
        if let PathArguments::AngleBracketed(args) = &mut segment.arguments {
            if !args
                .args
                .iter()
                .any(|a| matches!(a, GenericArgument::Lifetime(_)))
            {
                args.args
                    .insert(0, GenericArgument::Lifetime(lifetime.clone()));
            }
        }
    }
    Some(lifetime)
}

/// True if `ty`'s base ident is one of naclac's account-wrapper names
/// (`Account<T>`, `InterfaceAccount<T>` — see `core/src/wrappers/accounts/`).
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
