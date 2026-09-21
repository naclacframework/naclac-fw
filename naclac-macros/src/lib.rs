//! # Naclac Macros
//!
//! Procedural macros for the Naclac framework. This crate provides the core attributes and derives
//! necessary to build high-performance smart contracts.
//!
//! ## Supported Execution Modes
//! The macros dynamically generate code optimized for three distinct execution environments:
//! 1. **Solana Program + Borsh**: Traditional `std` mode using standard Borsh serialization.
//! 2. **Solana Program + Zero-Copy**: Highly optimized `std` mode utilizing native, zero-copy instruction parsing and custom heap allocation for dynamic types.
//! 3. **Pinocchio + Zero-Copy**: Strictly `no_std` mode built entirely on zero-copy principles for peak SBF performance and minimal compute.

extern crate proc_macro;

mod accounts;
mod checked_enum;
mod component;
mod error_code;
mod event;
mod instruction;
mod pod_struct_checks;
mod program;
mod system;
mod type_classify;

use proc_macro::TokenStream;

/// For attribute macros that take no argument at all — returns a
/// `compile_error!` if `attr` is non-empty, `None` otherwise.
fn reject_nonempty_attr(attr: &TokenStream, macro_name: &str) -> Option<TokenStream> {
    if attr.is_empty() {
        return None;
    }
    Some(
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("Naclac Error: #[{macro_name}] does not take any arguments."),
        )
        .to_compile_error()
        .into(),
    )
}

/// Emits a real compiler *warning* (not a hard error) for a heap-allocated
/// `Vec<T>`/`String` instruction argument used in a zero-copy program.
///
/// Unlike a `Vec<T>`/`String` field in a *persisted* `#[component]` (see
/// component.rs — that one is rejected outright, because a pointer cannot
/// survive being written to an account and read back in a later transaction),
/// a heap-allocated instruction argument is not unsound — it's a valid choice,
/// e.g. for interop with an existing API shaped around `Vec`/`String`. This is
/// a nudge toward `ZcVec<T>`/`ZcString` for the zero-copy-parsing CU savings,
/// not a ban, so it must be a warning a caller can silence, not a `compile_error!`.
///
/// Silenced via a purpose-built `#[allow_heap]` marker on the specific
/// argument (stripped and checked by `program.rs` before this ever gets
/// called), NOT `#[allow(deprecated)]` — the generic `deprecated` lint would
/// also swallow unrelated, real deprecation warnings anywhere else in the same
/// function. `#[deprecated]` is used only as the underlying mechanism to get a
/// real, visible `cargo build` warning out of stable proc-macro code; the
/// silencing story is entirely our own, not rustc's generic lint-allow system.
pub(crate) fn heap_collection_warning(
    arg_name: &str,
    found_ty: &str,
    suggested_ty: &str,
) -> proc_macro2::TokenStream {
    let warn_fn = quote::format_ident!("__naclac_heap_collection_warning_{}", arg_name);
    let note = format!(
        "Naclac: argument `{arg_name}` is `{found_ty}` in a zero-copy program — this heap-allocates \
         and copies on every deserialize. Consider `{suggested_ty}` for zero-copy (no-allocation) \
         parsing instead, or add `#[allow_heap]` to this argument to silence this warning."
    );
    quote::quote! {
        #[deprecated(note = #note)]
        #[allow(non_snake_case, dead_code)]
        fn #warn_fn() {}
        #warn_fn();
    }
}

/// Defines an on-chain account data structure.
///
/// In zero-copy mode (Solana zero-copy or Pinocchio), generates a `#[repr(C)]`
/// struct with `Pod`/`Zeroable`/`NaclacPod` impls for raw byte-cast access. In
/// Borsh mode, generates standard `BorshSerialize`/`BorshDeserialize` impls instead.
#[proc_macro_attribute]
pub fn component(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Some(err) = reject_nonempty_attr(&attr, "component") {
        return err;
    }
    component::expand(attr, item)
}

/// Compile-time math-safety engine for instruction logic functions: bans
/// floats, checked-math-rewrites arithmetic operators, catches literal
/// overflow/div-by-zero at compile time, and (via attribute args) supports
/// rounding control, decimal-scale tracking, proptest fuzz-test generation,
/// and Kani proof-harness generation. See `system.rs`'s module docs for the
/// full attribute syntax.
#[proc_macro_attribute]
pub fn system(attr: TokenStream, item: TokenStream) -> TokenStream {
    system::expand(attr, item)
}

/// The main entrypoint macro for a Naclac smart contract.
///
/// This generates the overarching instruction dispatcher and handles Cross-Program
/// Invocation (CPI) module generation. Zero-copy vs Borsh mode is auto-detected
/// from the crate's own feature flags, not from an argument here.
#[proc_macro_attribute]
pub fn program(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Some(err) = reject_nonempty_attr(&attr, "program") {
        return err;
    }
    program::expand(attr, item)
}

/// Generates standardized custom error codes for the program.
#[proc_macro_attribute]
pub fn error_code(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Some(err) = reject_nonempty_attr(&attr, "error_code") {
        return err;
    }
    error_code::expand(attr, item)
}

/// Defines an event struct.
///
/// If zero-copy is active, generates highly optimized, allocator-free event emission
/// logic directly into the Solana runtime via `sol_invoke`.
#[proc_macro_attribute]
pub fn event(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    event::expand(attr, item)
}

/// Builds a plain (never `#[test]`-tagged — a `[[bin]]` target's `main()`
/// can't invoke a `#[test]` function; those only exist in a separate
/// `cargo test` harness binary) print function for one `#[constant]`-tagged
/// const, returning its real `IdlConstant` — mirrors Anchor's own
/// `lang/syn/src/idl/constant.rs::gen_idl_print_fn_constant`, minus the
/// `#[test]` wrapper. The constant's *type* is resolved locally (a
/// top-level const's own declared type needs no cross-item visibility,
/// same as any other field's own type elsewhere in this migration); its
/// *value* is read via `format!("{:?}", #expr)` — real compiled Rust,
/// evaluated when this function actually runs at `idl-build` time, not
/// text-parsed. `#[program]`'s own macro is what discovers this function's
/// name (a crate-wide scan, which a single `#[constant]` invocation has no
/// visibility to do itself) and generates the call into `main()`.
#[cfg(feature = "idl-build")]
fn constant_idl_build_impl(item: &syn::ItemConst) -> proc_macro2::TokenStream {
    let name = item.ident.to_string();
    let expr = &item.expr;
    let docs = naclac_syn::parser::extract_docs(&item.attrs);
    let ty_value = naclac_syn::parser::rust_type_to_idl(&item.ty, &[]);
    let Ok(ty_json) = serde_json::to_string(&ty_value) else {
        return quote::quote! {};
    };
    // Lowercased — a `const`'s own name is conventionally SCREAMING_SNAKE_CASE,
    // which would otherwise make this generated function name trip
    // `non_snake_case`; `#[program]`'s scan-generated call
    // (`program.rs::const_calls`) lowercases the same way to match.
    let fn_name = quote::format_ident!(
        "__naclac_idl_print_const_{}",
        item.ident.to_string().to_lowercase()
    );
    quote::quote! {
        #[cfg(feature = "idl-build")]
        #[doc(hidden)]
        pub fn #fn_name() -> naclac_lang::naclac_idl::IdlConstant {
            naclac_lang::naclac_idl::IdlConstant {
                name: #name.into(),
                docs: vec![#(#docs.into()),*],
                ty: naclac_lang::naclac_idl::serde_json::from_str(#ty_json)
                    .expect("naclac idl-build: generated constant type JSON must parse"),
                value: format!("{:?}", #expr),
            }
        }
    }
}

/// Stub for constant declarations (ignored, acts as a marker) — `naclac-syn`'s
/// AST-walker IDL generator scans for this attribute's bare presence on a
/// `const` item. Under `idl-build`, also generates that constant's own
/// print function (see `constant_idl_build_impl`); the attribute itself
/// never modifies the original `const` item either way.
#[proc_macro_attribute]
pub fn constant(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Some(err) = reject_nonempty_attr(&attr, "constant") {
        return err;
    }
    #[cfg(feature = "idl-build")]
    {
        let input = syn::parse_macro_input!(item as syn::ItemConst);
        let print_fn = constant_idl_build_impl(&input);
        quote::quote! {
            #input
            #print_fn
        }
        .into()
    }
    #[cfg(not(feature = "idl-build"))]
    {
        item
    }
}

/// Derives the necessary validation, deserialization, and constraints for an Accounts struct.
///
/// Generates zero-copy fixed array CPI properties (`[AccountMeta; N]`) regardless of the execution mode.
#[proc_macro_derive(Accounts, attributes(account, instruction))]
pub fn derive_accounts(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    accounts::expand_derive_accounts(item)
}

/// Derives the correct serialization mechanism for a standalone, non-account,
/// non-event type (a struct/enum used as e.g. an instruction arg or a nested
/// field) — matching the same automatic per-mode routing `#[component]`/
/// `#[event]` already use: zero-copy mode gets `Pod`/`Zeroable` (structs) or
/// `CheckedBitPattern` (enums) for byte-level casting; Borsh mode gets real
/// `borsh::BorshSerialize`/`BorshDeserialize`.
///
/// The zero-copy struct/enum split exists because every bit pattern of a
/// fixed-size struct's bytes is a valid value (safe to declare unconditionally
/// `Pod`), but an enum's discriminant only has as many valid values as it has
/// variants — reinterpreting an out-of-range discriminant byte as the enum is
/// undefined behavior. Enums therefore get bytemuck's own `CheckedBitPattern`
/// (validated on read) instead of `Pod`/`Zeroable` (unconditional), and their
/// `NaclacPod::naclac_from_bytes` validates the discriminant and panics on an
/// invalid one rather than blindly transmuting it — a panic is safe, defined
/// behavior, unlike constructing an enum value with no corresponding variant.
/// Borsh mode needs no such split — real Borsh's own derive already validates
/// enum tags during deserialization, for structs and enums alike.
#[proc_macro_attribute]
pub fn defined_type(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    if let Some(err) = reject_nonempty_attr(&attr, "defined_type") {
        return err;
    }
    let mut input = syn::parse_macro_input!(item as syn::DeriveInput);
    let ident = input.ident.clone();
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let is_enum = matches!(&input.data, syn::Data::Enum(_));
    // Captured before any field-padding mutation below — the IDL-visible
    // shape reflects what the developer actually wrote, not the on-chain
    // zero-copy layout's auto-added padding field.
    let original_data = input.data.clone();

    // Cargo gives a proc-macro no reliable way to see the invoking crate's
    // activated features (see `component.rs`'s doc comment for the full
    // story), so both representations are always emitted below, each gated
    // by a real `#[cfg(...)]` in the output that the calling crate's own
    // compiler resolves — same as `#[component]`/`#[event]`/`#[derive(Accounts)]`.
    let zero_copy_cfg = quote::quote! { #[cfg(any(feature = "pinocchio", not(feature = "borsh")))] };
    let borsh_cfg = quote::quote! { #[cfg(all(not(feature = "pinocchio"), feature = "borsh"))] };

    // --- Borsh variant ---
    // No repr/Copy forcing, no Pod-family traits at all — real Borsh's own
    // derive handles structs and enums (including data-carrying variants)
    // natively, with its own tag validation.
    let borsh_variant = {
        let mut input = input.clone();
        let mut has_clone = false;
        for attr in &input.attrs {
            if !attr.path().is_ident("derive") {
                continue;
            }
            let Ok(paths) = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            ) else {
                continue;
            };
            has_clone |= paths
                .iter()
                .any(|p| p.segments.last().is_some_and(|s| s.ident == "Clone"));
        }
        if !has_clone {
            input.attrs.push(syn::parse_quote!(#[derive(Clone)]));
        }
        quote::quote! {
            #borsh_cfg
            #[derive(naclac_lang::prelude::BorshSerialize, naclac_lang::prelude::BorshDeserialize)]
            #[borsh(crate = "naclac_lang::prelude::borsh")]
            #input
        }
    };

    // --- Zero-copy variant ---
    // Auto-add a deterministic layout repr when the caller hasn't picked one
    // explicitly: #[repr(u8)] for enums, #[repr(C)] for structs.
    match &input.data {
        // An enum's generated `bytemuck::CheckedBitPattern` layout below is
        // sized to a concrete integer discriminant width — a bare
        // `#[repr(C)]` (or no repr at all) doesn't commit to one, so
        // `#[repr(u8)]` must be added whenever no *explicit integer* repr is
        // present, not merely whenever no repr at all is present. Pushing
        // `#[repr(u8)]` alongside an existing `#[repr(C)]` is valid Rust —
        // multiple `#[repr(...)]` attributes on one item combine, same as
        // writing `#[repr(C, u8)]` — so this is safe regardless of what
        // other repr modifiers are already there. Must use the same
        // detection `tag_ty` below uses, and that `naclac_syn::discriminator
        // ::detect_enum_repr` (the IDL walker's own detector) uses — all
        // three now share one function so they can't drift apart again.
        syn::Data::Enum(_) => {
            if naclac_syn::discriminator::detect_enum_repr(&input.attrs).is_none() {
                input.attrs.push(syn::parse_quote!(#[repr(u8)]));
            }
        }
        syn::Data::Struct(_) => {
            let has_repr = input.attrs.iter().any(|attr| attr.path().is_ident("repr"));
            if !has_repr {
                input.attrs.push(syn::parse_quote!(#[repr(C)]));
            }
        }
        syn::Data::Union(_) => {}
    }

    // `Pod` requires `Copy` as a supertrait unconditionally — there is no
    // scenario where a Pod type doesn't need Copy — so auto-add
    // #[derive(Clone, Copy)] unless the caller already derives them. Checked
    // across every #[derive(...)] attribute on the item (not just the
    // first), since `Clone`/`Copy` can legally be written as two separate
    // attributes rather than one combined `#[derive(Clone, Copy)]`.
    let mut has_clone = false;
    let mut has_copy = false;
    for attr in &input.attrs {
        if !attr.path().is_ident("derive") {
            continue;
        }
        let Ok(paths) = attr.parse_args_with(
            syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
        ) else {
            continue;
        };
        for path in &paths {
            if let Some(seg) = path.segments.last() {
                has_clone |= seg.ident == "Clone";
                has_copy |= seg.ident == "Copy";
            }
        }
    }
    // Add only whichever of the two is actually missing — re-deriving one
    // that's already present (e.g. a type with `#[derive(Clone)]` alone)
    // would be a duplicate-derive compile error, not a no-op.
    if has_clone && !has_copy {
        input.attrs.push(syn::parse_quote!(#[derive(Copy)]));
    } else if has_copy && !has_clone {
        input.attrs.push(syn::parse_quote!(#[derive(Clone)]));
    } else if !has_clone && !has_copy {
        input.attrs.push(syn::parse_quote!(#[derive(Clone, Copy)]));
    }

    let zero_copy_variant = if is_enum {
        // No `derive(Copy)`/`derive(Clone)` re-added here — already pushed
        // onto `input.attrs` above, and `bytemuck::CheckedBitPattern`
        // requires `Copy` as a supertrait just like `Pod` does.
        let syn::Data::Enum(data_enum) = &input.data else {
            unreachable!("is_enum was computed from input.data being Data::Enum")
        };
        let tag_ty: syn::Type = {
            let named = naclac_syn::discriminator::detect_enum_repr(&input.attrs);
            syn::parse_str(named.as_deref().unwrap_or("u8")).unwrap()
        };
        let (extra_items, checked_bit_pattern_impl) =
            checked_enum::generate(&ident, &input.vis, &data_enum.variants, &tag_ty, &zero_copy_cfg);
        quote::quote! {
            #zero_copy_cfg
            #input

            #extra_items
            #checked_bit_pattern_impl

            #zero_copy_cfg
            impl #impl_generics naclac_lang::prelude::NaclacPod for #ident #ty_generics #where_clause {
                #[inline(always)]
                fn naclac_from_bytes(data: &[u8]) -> Self {
                    // Unlike a struct (every byte pattern valid), an enum's
                    // discriminant must be checked — see this macro's own
                    // doc comment. `try_pod_read_unaligned` (unlike
                    // `try_from_bytes`) validates the discriminant without
                    // requiring proper alignment, which real Solana
                    // account/instruction byte buffers don't guarantee.
                    // `expect` turns an invalid discriminant into a safe
                    // panic instead of undefined behavior.
                    naclac_lang::prelude::bytemuck::checked::try_pod_read_unaligned::<Self>(
                        &data[..core::mem::size_of::<Self>()],
                    )
                    .expect("defined_type: invalid enum discriminant in account/instruction bytes")
                }
                #[inline(always)]
                fn naclac_size() -> usize {
                    core::mem::size_of::<Self>()
                }
            }
        }
    } else {
        let syn::Data::Struct(data_struct) = &input.data else {
            unreachable!("is_enum was false, so input.data must be Data::Struct")
        };
        let is_tuple = matches!(&data_struct.fields, syn::Fields::Unnamed(_));
        // A user-written `#[derive(Default)]` is evaluated against the
        // struct's *original* field list — by the time the auto-padding
        // field is spliced in below, the derived code would construct a
        // value missing it, a real E0063 hit on `pump-fees`'s own `Fees`
        // struct. `event.rs` already solves this correctly (hand-written
        // `impl Default` via `Zeroable::zeroed()`, not `#[derive(Default)]`
        // — `std`'s derive doesn't even support arrays past length 32
        // anyway); `defined_type` does the same, so a user-written
        // `#[derive(Default)]` is now always redundant and must be removed
        // rather than silently working around it.
        for attr in &input.attrs {
            if !attr.path().is_ident("derive") {
                continue;
            }
            let Ok(paths) = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            ) else {
                continue;
            };
            if paths
                .iter()
                .any(|p| p.segments.last().is_some_and(|s| s.ident == "Default"))
            {
                let err = syn::Error::new_spanned(
                    attr,
                    "Naclac Error: `#[defined_type]` already generates `Default` (via \
                     `Zeroable::zeroed()`) for every struct — remove this `#[derive(Default)]`, \
                     it's redundant and, once an auto-padding field is added, incorrect (`std`'s \
                     derive is evaluated before that field exists).",
                )
                .to_compile_error();
                return quote::quote! {
                    #input
                    #err
                }
                .into();
            }
        }

        let (padded_fields, pod_checks) =
            pod_struct_checks::generate(&ident, &data_struct.fields, &zero_copy_cfg);
        // Splice the auto-padded field list back into `input` itself (rather
        // than re-declaring the struct by hand) so every other attribute on
        // it (doc comments, other derives, etc.) survives unchanged. No
        // trailing `;` to worry about either way — `input`'s own semicolon
        // token (present only for a tuple struct) already survives
        // untouched, since only `.fields` itself is being replaced here.
        let syn::Data::Struct(data_struct) = &mut input.data else {
            unreachable!("checked above")
        };
        data_struct.fields = if is_tuple {
            syn::Fields::Unnamed(
                syn::parse2(padded_fields)
                    .expect("defined_type: generated padded tuple field list must parse"),
            )
        } else {
            syn::Fields::Named(
                syn::parse2(padded_fields)
                    .expect("defined_type: generated padded named field list must parse"),
            )
        };
        quote::quote! {
            #zero_copy_cfg
            #input

            #pod_checks

            // SAFETY: sound only because the annotated type has a deterministic
            // layout (#[repr(u8)]/#[repr(C)]) and is Copy, both auto-added above
            // when the caller hasn't already provided them — and because
            // `pod_checks` (above) verifies at compile time that the struct
            // has no internal padding gap and every declared field is itself
            // Pod. The auto-added trailing `_padding` field (spliced into
            // `input` above) closes the one gap this check can safely
            // account for on its own.
            #zero_copy_cfg
            unsafe impl #impl_generics naclac_lang::prelude::Pod for #ident #ty_generics #where_clause {}
            // SAFETY: Zeroed memory is a valid initial state for Pod types.
            #zero_copy_cfg
            unsafe impl #impl_generics naclac_lang::prelude::Zeroable for #ident #ty_generics #where_clause {}

            // Not `#[derive(Default)]` (rejected above if the caller wrote
            // it themselves) — `std`'s derive only implements `Default` for
            // arrays up to length 32, which a `[u8; N]` padding field (or
            // any other large fixed-size field) can easily exceed.
            // `Zeroable` (already required above) has no such limit, and
            // lets every existing `Struct { field: v, ..Default::default() }`
            // construction site keep working without knowing about the
            // auto-inserted padding field at all.
            #zero_copy_cfg
            impl #impl_generics core::default::Default for #ident #ty_generics #where_clause {
                fn default() -> Self {
                    naclac_lang::prelude::bytemuck::Zeroable::zeroed()
                }
            }

            #zero_copy_cfg
            impl #impl_generics naclac_lang::prelude::NaclacPod for #ident #ty_generics #where_clause {
                #[inline(always)]
                fn naclac_from_bytes(data: &[u8]) -> Self {
                    // SAFETY: We use `read_unaligned` to safely read the struct from the byte slice.
                    // This is required because SBF instruction data streams may not align properly
                    // to the struct's natural alignment boundaries, preventing standard bytemuck casting.
                    unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Self) }
                }
                #[inline(always)]
                fn naclac_size() -> usize {
                    core::mem::size_of::<Self>()
                }
            }
        }
    };

    // `input.attrs` here (not `original_data`) is deliberate for the enum
    // case — it reflects the *final* repr after the auto-`#[repr(u8)]`
    // default above, matching the enum's real compiled discriminant width,
    // not just whatever the developer happened to write (or omit).
    let idl_build_variant = match &original_data {
        syn::Data::Struct(s) => crate::component::idl_build_impl(&ident, &s.fields),
        syn::Data::Enum(e) => idl_build_impl_enum(&ident, &e.variants, &input.attrs),
        syn::Data::Union(_) => quote::quote! {},
    };

    quote::quote! {
        #borsh_variant
        #zero_copy_variant
        #idl_build_variant
    }
    .into()
}

/// Builds `impl NaclacIdlBuild for #ident` for a `#[defined_type]` enum —
/// same principle as `component::idl_build_impl` (built once from the
/// already-parsed `syn::Variant`s, no disk re-read), but producing
/// `IdlTypeDef::Enum { variants, repr }` instead of `Struct { fields }`.
/// `attrs` must be the enum's *final* attribute list (after any
/// auto-added `#[repr(u8)]` default — see the call site), since the IDL's
/// recorded repr must match the enum's real compiled discriminant width.
#[cfg(feature = "idl-build")]
fn idl_build_impl_enum(
    ident: &syn::Ident,
    variants: &syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>,
    attrs: &[syn::Attribute],
) -> proc_macro2::TokenStream {
    let repr = naclac_syn::discriminator::detect_enum_repr(attrs);
    let repr_tokens = match &repr {
        Some(r) => quote::quote! { Some(#r.to_string()) },
        None => quote::quote! { None },
    };

    let discriminants = naclac_syn::discriminator::variant_discriminants(variants.iter());
    let mut idl_variants = Vec::new();
    let mut defined_refs: Vec<syn::Type> = Vec::new();

    for (variant, discriminant) in variants.iter().zip(discriminants.iter()) {
        let variant_name = variant.ident.to_string();
        let discriminant_str = discriminant.to_string();
        let variant_docs = naclac_syn::parser::extract_docs(&variant.attrs);

        let fields_ts = match &variant.fields {
            syn::Fields::Unit => quote::quote! { None },
            syn::Fields::Named(named) => {
                let mut field_ts_list = Vec::new();
                for f in &named.named {
                    let fname = f.ident.as_ref().unwrap().to_string();
                    let field_docs = naclac_syn::parser::extract_docs(&f.attrs);
                    let ty_value = naclac_syn::parser::rust_type_to_idl(&f.ty, &[]);
                    let Ok(ty_json) = serde_json::to_string(&ty_value) else {
                        continue;
                    };
                    field_ts_list.push(quote::quote! {
                        naclac_lang::naclac_idl::IdlField {
                            name: #fname.into(),
                            docs: vec![#(#field_docs.into()),*],
                            ty: naclac_lang::naclac_idl::serde_json::from_str(#ty_json)
                                .expect("naclac idl-build: generated field type JSON must parse"),
                        }
                    });
                    if let Some(defined_ty) = naclac_syn::parser::defined_type_leaf(&f.ty) {
                        defined_refs.push(defined_ty);
                    }
                }
                quote::quote! {
                    Some(naclac_lang::naclac_idl::IdlEnumFields::Named(vec![#(#field_ts_list),*]))
                }
            }
            syn::Fields::Unnamed(unnamed) => {
                let mut ty_ts_list = Vec::new();
                for f in &unnamed.unnamed {
                    let ty_value = naclac_syn::parser::rust_type_to_idl(&f.ty, &[]);
                    let Ok(ty_json) = serde_json::to_string(&ty_value) else {
                        continue;
                    };
                    ty_ts_list.push(quote::quote! {
                        naclac_lang::naclac_idl::serde_json::from_str(#ty_json)
                            .expect("naclac idl-build: generated field type JSON must parse")
                    });
                    if let Some(defined_ty) = naclac_syn::parser::defined_type_leaf(&f.ty) {
                        defined_refs.push(defined_ty);
                    }
                }
                quote::quote! {
                    Some(naclac_lang::naclac_idl::IdlEnumFields::Tuple(vec![#(#ty_ts_list),*]))
                }
            }
        };

        idl_variants.push(quote::quote! {
            naclac_lang::naclac_idl::IdlEnumVariant {
                name: #variant_name.into(),
                docs: vec![#(#variant_docs.into()),*],
                fields: #fields_ts,
                discriminant: #discriminant_str.into(),
            }
        });
    }

    let ident_str = ident.to_string();

    quote::quote! {
        #[cfg(feature = "idl-build")]
        impl naclac_lang::naclac_idl::idl_build::NaclacIdlBuild for #ident {
            fn create_type() -> Option<naclac_lang::naclac_idl::IdlTypeDef> {
                Some(naclac_lang::naclac_idl::IdlTypeDef::Enum {
                    variants: vec![#(#idl_variants),*],
                    repr: #repr_tokens,
                })
            }

            fn insert_types(
                types: &mut naclac_lang::naclac_idl::__private::BTreeMap<
                    naclac_lang::naclac_idl::__private::String,
                    naclac_lang::naclac_idl::IdlTypeDef,
                >,
            ) {
                #(
                    if let Some(ty) = <#defined_refs as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::create_type() {
                        types.insert(
                            <#defined_refs as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::get_full_path(),
                            ty,
                        );
                        <#defined_refs as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::insert_types(types);
                    }
                )*
            }

            fn get_full_path() -> naclac_lang::naclac_idl::__private::String {
                #ident_str.into()
            }
        }
    }
}

#[cfg(not(feature = "idl-build"))]
fn idl_build_impl_enum(
    _ident: &syn::Ident,
    _variants: &syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>,
    _attrs: &[syn::Attribute],
) -> proc_macro2::TokenStream {
    quote::quote! {}
}

/// Generates a statement binding `field_name` by reading it off `data`
/// starting at `*offset`, advancing `*offset` past the consumed bytes.
/// Mirrors `program.rs`'s own per-argument dispatch (the same wire format:
/// a fixed field is read via `NaclacPod` at its known size; a dynamic
/// (`ZcString`/`Span<T>`/`Vec<T>`/`String`) field is prefixed with a
/// little-endian `u32` *byte* length, matching `naclac-client-gen`'s
/// `generate_zero_copy_arg_bytes` on the write side) — required because a
/// dynamic field can never be read via a single raw `size_of`-based cast the
/// way `NaclacPod::naclac_from_bytes` reads a fixed field.
fn field_deserialize_stmt(
    field_name: &syn::Ident,
    field_ty: &syn::Type,
) -> proc_macro2::TokenStream {
    use type_classify::DynamicKind;
    match type_classify::classify_dynamic(field_ty) {
        DynamicKind::Fixed => quote::quote! {
            let #field_name = {
                let __sz = <#field_ty as naclac_lang::prelude::NaclacPod>::naclac_size();
                if data.len() < *offset + __sz {
                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                }
                let __val = <#field_ty as naclac_lang::prelude::NaclacPod>::naclac_from_bytes(
                    &data[*offset..*offset + __sz]
                );
                *offset += __sz;
                __val
            };
        },
        DynamicKind::ZcString => quote::quote! {
            let #field_name = {
                if data.len() < *offset + 4 {
                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                }
                let mut __len_bytes = [0u8; 4];
                __len_bytes.copy_from_slice(&data[*offset..*offset + 4]);
                let __len = u32::from_le_bytes(__len_bytes) as usize;
                *offset += 4;
                if data.len() < *offset + __len {
                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                }
                let __val = <naclac_lang::prelude::ZcString>::from_bytes(&data[*offset..*offset + __len])?;
                *offset += __len;
                __val
            };
        },
        DynamicKind::ZcVec => {
            let inner = type_classify::first_generic_type(field_ty);
            quote::quote! {
                let #field_name = {
                    if data.len() < *offset + 4 {
                        return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                    }
                    let mut __len_bytes = [0u8; 4];
                    __len_bytes.copy_from_slice(&data[*offset..*offset + 4]);
                    let __len = u32::from_le_bytes(__len_bytes) as usize;
                    *offset += 4;
                    if data.len() < *offset + __len {
                        return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                    }
                    let __val = <naclac_lang::prelude::Span<#inner>>::from_bytes(&data[*offset..*offset + __len])?;
                    *offset += __len;
                    __val
                };
            }
        }
        DynamicKind::HeapString => quote::quote! {
            let #field_name = {
                if data.len() < *offset + 4 {
                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                }
                let mut __len_bytes = [0u8; 4];
                __len_bytes.copy_from_slice(&data[*offset..*offset + 4]);
                let __len = u32::from_le_bytes(__len_bytes) as usize;
                *offset += 4;
                if data.len() < *offset + __len {
                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                }
                let __val = naclac_lang::prelude::String::from_utf8(data[*offset..*offset + __len].to_vec())
                    .map_err(|_| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?;
                *offset += __len;
                __val
            };
        },
        DynamicKind::HeapVec => quote::quote! {
            let #field_name = {
                if data.len() < *offset + 4 {
                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                }
                let mut __len_bytes = [0u8; 4];
                __len_bytes.copy_from_slice(&data[*offset..*offset + 4]);
                let __len = u32::from_le_bytes(__len_bytes) as usize;
                *offset += 4;
                if data.len() < *offset + __len {
                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                }
                let mut __val = naclac_lang::prelude::Vec::with_capacity(__len);
                __val.extend_from_slice(&data[*offset..*offset + __len]);
                *offset += __len;
                __val
            };
        },
    }
}

/// Proc macro attribute for grouping instruction arguments.
/// Implements repr(C), Clone, Copy, Pod, Zeroable, NaclacPod, and optionally Borsh serialize/deserialize.
#[proc_macro_attribute]
pub fn instruction_args(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    if let Some(err) = reject_nonempty_attr(&attr, "instruction_args") {
        return err;
    }
    let input = syn::parse_macro_input!(item as syn::DeriveInput);
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let syn::Data::Struct(data_struct) = &input.data else {
        panic!("Naclac: #[instruction_args] only supports structs");
    };
    let syn::Fields::Named(fields_named) = &data_struct.fields else {
        panic!("Naclac: #[instruction_args] requires named fields");
    };
    let has_dynamic_field = fields_named.named.iter().any(|f| {
        !matches!(
            type_classify::classify_dynamic(&f.ty),
            type_classify::DynamicKind::Fixed
        )
    });
    // `ZcVec<T>`/`Span<T>`/`ZcString` are zero-copy *views* (pointer +
    // length) into the current instruction's byte buffer — they have no
    // bytes of their own to serialize, so deriving Borsh on a struct that
    // contains one is unsound-by-construction, not just unsupported. Real
    // Borsh already decodes `Vec<T>`/`String` natively, so that's the
    // correct replacement in Borsh mode, not a `ZcVec`/`ZcString` field.
    let zc_field = fields_named.named.iter().find(|f| {
        matches!(
            type_classify::classify_dynamic(&f.ty),
            type_classify::DynamicKind::ZcVec | type_classify::DynamicKind::ZcString
        )
    });

    // A fixed-field args struct always gets the Pod/Zeroable path below,
    // regardless of representation — Borsh derives are only ever
    // *additionally* layered on top when the calling crate is genuinely in
    // Borsh mode, not an exclusive alternative to it, so (unlike
    // `#[component]`/`#[event]`/`#[derive(Accounts)]`) this doesn't need
    // full item duplication — a `cfg_attr` is enough, real-cfg-gated so the
    // calling crate's own compiler resolves it (Cargo gives a proc-macro no
    // reliable way to see the invoking crate's activated features — see
    // `component.rs`'s doc comment). Except when `zc_field` is present: a
    // `ZcVec<T>`/`ZcString` field can never be Borsh-derived, so the derive
    // is skipped entirely for such a struct and a clear, purpose-built error
    // takes its place instead of the confusing raw trait-bound error rustc
    // would otherwise give — same real-cfg-gating, only fires when the
    // calling crate actually selects Borsh mode.
    let (borsh_derive_attr, borsh_error) = if let Some(field) = zc_field {
        let field_name = field.ident.as_ref().expect("named field");
        let msg = format!(
            "Naclac Error: field '{}' uses a zero-copy-only type ('ZcVec'/'Span<T>'/'ZcString') \
             in a Borsh-mode #[instruction_args]. These are views (pointer + length) into the \
             current instruction's byte buffer with no bytes of their own to serialize — real \
             Borsh already decodes 'Vec<T>'/'String' natively, so use one of those instead.",
            field_name
        );
        (
            quote::quote! {},
            quote::quote! { #[cfg(feature = "borsh")] compile_error!(#msg); },
        )
    } else {
        // NOTE: this must be *real*, field-by-field Borsh encoding (via the
        // actual `borsh` derive macros), not a raw `size_of::<Self>()`
        // memory copy. A raw memcpy silently includes any repr(C) alignment
        // padding, which diverges from the client SDK's real (tightly-packed)
        // Borsh encoding the moment a struct's field order forces the
        // compiler to insert padding — e.g. a u64 following a run of u8
        // fields. That divergence is a wire-format mismatch, not a
        // compile-time-detectable one, so it only surfaces as a runtime
        // `InvalidInstructionData` deserialization failure on-chain.
        (
            quote::quote! {
                #[cfg_attr(feature = "borsh", derive(naclac_lang::prelude::BorshSerialize, naclac_lang::prelude::BorshDeserialize))]
                #[cfg_attr(feature = "borsh", borsh(crate = "naclac_lang::prelude::borsh"))]
            },
            quote::quote! {},
        )
    };
    let zero_copy_cfg = quote::quote! { #[cfg(any(feature = "pinocchio", not(feature = "borsh")))] };

    // `#[instruction_args]` structs are directly usable as an instruction
    // arg's own type, so `#[program]`'s idl-build assembler chases them the
    // same way it chases any other non-primitive arg type
    // (`naclac_syn::parser::defined_type_leaf`) — without this, that chase
    // would generate `<Self as NaclacIdlBuild>::create_type()` for a type
    // that never implements the trait, a hard compile error the moment a
    // real program used `#[instruction_args]` under `idl-build`. Reuses
    // `component.rs`'s own `idl_build_impl` directly rather than
    // duplicating it — same named-fields shape, same per-field type mapping.
    let idl_build_impl_tokens = component::idl_build_impl(ident, &data_struct.fields);

    if !has_dynamic_field {
        let expanded = quote::quote! {
            #[repr(C)]
            #[derive(Clone, Copy)]
            #borsh_derive_attr
            #input

            unsafe impl naclac_lang::prelude::Pod for #ident #ty_generics #where_clause {}
            unsafe impl naclac_lang::prelude::Zeroable for #ident #ty_generics #where_clause {}

            impl #impl_generics naclac_lang::prelude::NaclacPod for #ident #ty_generics #where_clause {
                #[inline(always)]
                fn naclac_from_bytes(data: &[u8]) -> Self {
                    unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Self) }
                }
                #[inline(always)]
                fn naclac_size() -> usize {
                    core::mem::size_of::<Self>()
                }
            }

            #idl_build_impl_tokens
        };
        return expanded.into();
    }

    // Zero-copy mode only — in Borsh mode, `program.rs`'s dispatch reads this
    // arg via `BorshDeserialize::deserialize` directly (never `NaclacArgs`),
    // and the derived `BorshSerialize`/`BorshDeserialize` above already
    // handles `String`/`Vec<T>` fields natively. Real-cfg-gated in the
    // output (see `borsh_derive_attr`'s comment) rather than decided here.
    let naclac_args_impl = {
        let field_names: Vec<&syn::Ident> = fields_named
            .named
            .iter()
            .map(|f| f.ident.as_ref().expect("named field"))
            .collect();
        let field_reads: Vec<proc_macro2::TokenStream> = fields_named
            .named
            .iter()
            .map(|f| field_deserialize_stmt(f.ident.as_ref().expect("named field"), &f.ty))
            .collect();

        quote::quote! {
            #zero_copy_cfg
            impl #impl_generics naclac_lang::prelude::NaclacArgs for #ident #ty_generics #where_clause {
                fn naclac_deserialize(
                    data: &[u8],
                    offset: &mut usize,
                ) -> naclac_lang::prelude::Result<Self> {
                    #( #field_reads )*
                    Ok(Self { #( #field_names ),* })
                }
            }
        }
    };

    let expanded = quote::quote! {
        #borsh_error

        #[derive(Clone)]
        #borsh_derive_attr
        #input

        #naclac_args_impl
        #idl_build_impl_tokens
    };
    expanded.into()
}
