//! # Naclac Macros
//!
//! Procedural macros for the Naclac framework. This crate provides the core attributes and derives
//! necessary to build high-performance smart contracts.
//!
//! ## Supported Execution Modes
//! A program picks a runtime (`solana` or `pinocchio`) and one account-data layout
//! (`bytemuck`, `wincode` or `borsh`), independently of each other:
//! - **`bytemuck`** (zero-copy): native, zero-copy instruction parsing and account access, on either runtime.
//! - **`wincode`**: heap-allocated accounts and arguments serialized with Wincode, on either runtime.
//! - **`borsh`**: heap-allocated accounts and arguments serialized with Borsh, on the `solana` runtime only.

extern crate proc_macro;

mod accounts;
mod component;
mod constraint_args;
mod emit;
mod error_code;
mod event;
mod event_cpi;
mod instruction;
mod pod_struct_checks;
mod program;
mod serialized_bounds;
mod system;
mod type_classify;
mod zc_enum;

use proc_macro::TokenStream;

/// The directory of the crate being compiled, from `CARGO_MANIFEST_DIR`. The
/// macros read that crate's own source for checks that cannot be done from
/// the item being expanded, so a missing variable is an error: continuing
/// without it would silently skip those checks.
pub(crate) fn manifest_dir() -> std::path::PathBuf {
    std::env::var_os("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            panic!(
                "naclac: CARGO_MANIFEST_DIR is not set; the naclac macros must be expanded by \
                 cargo (or a tool that sets it, such as rust-analyzer)"
            )
        })
}

/// The calling crate's constants, for resolving `[T; N]` lengths when
/// building `idl-build` field types.
#[cfg(feature = "idl-build")]
pub(crate) fn crate_constants() -> Vec<naclac_syn::types::NaclacConstant> {
    naclac_syn::crate_constants(&manifest_dir())
}

/// The calling crate's type aliases, for the checks that match a field's type
/// by name.
pub(crate) fn crate_aliases() -> naclac_syn::aliases::Aliases {
    naclac_syn::aliases::Aliases::collect(&naclac_syn::modules::crate_modules(&manifest_dir()))
}

/// Tokens evaluating to the IDL `value` string of a constant of type `ty`
/// whose value is `expr`: an `Address` as base58 on every backend, anything
/// else as its `Debug` text.
#[cfg(feature = "idl-build")]
pub(crate) fn constant_value_tokens(
    ty: &syn::Type,
    expr: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if type_classify::is_exactly(ty, "Address") {
        quote::quote! { naclac_lang::prelude::base58::encode_address_to_string(&#expr) }
    } else {
        quote::quote! { format!("{:?}", #expr) }
    }
}

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
/// In zero-copy mode (the `bytemuck` layout, on either runtime), generates a `#[repr(C)]`
/// struct with `Pod`/`Zeroable`/`NaclacPod` impls for raw byte-cast access. In
/// Borsh mode, generates standard `BorshSerialize`/`BorshDeserialize` impls instead.
///
/// Takes an optional `discriminator = <int | [ints]>` argument (1 to 8 bytes)
/// replacing the default name-derived 8-byte discriminator.
#[proc_macro_attribute]
pub fn component(attr: TokenStream, item: TokenStream) -> TokenStream {
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

/// Generates standardized custom error codes for the program: each variant's
/// code is `offset + discriminant`, where `offset` is 6000 unless set with
/// `#[error_code(offset = N)]`. Every code in the crate must be unique
/// (`#[program]` rejects overlaps).
#[proc_macro_attribute]
pub fn error_code(attr: TokenStream, item: TokenStream) -> TokenStream {
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

/// Auto-injects the `event_authority`/`program` accounts a self-CPI event
/// emission (`emit_cpi!()`) needs. Must be applied *above*
/// `#[derive(Accounts)]`, not below it — as a real attribute macro it
/// rewrites the struct's own field list before the derive macro ever sees
/// it, which a derive macro alone cannot do.
#[proc_macro_attribute]
pub fn event_cpi(attr: TokenStream, item: TokenStream) -> TokenStream {
    event_cpi::expand(attr, item)
}

/// Logs an event with `sol_log_data`: `emit!(MyEvent { a, b: 2 });` or
/// `emit!(event_value);`. A struct literal starts from `Default` and assigns
/// each written field through `.into()`. Evaluates to the event.
#[proc_macro]
pub fn emit(input: TokenStream) -> TokenStream {
    emit::expand_emit(input)
}

/// Emits an event as a signed self-CPI: `emit_cpi!(MyEvent { a, b: 2 });`.
/// Takes exactly the event syntax `emit!` takes and evaluates to the event.
/// The handler's context is the variable named `ctx`; a function holding its
/// context under another name calls
/// `naclac_lang::event::emit_cpi(&context, &event)` directly. The handler's
/// accounts struct must carry `#[event_cpi]`.
#[proc_macro]
pub fn emit_cpi(input: TokenStream) -> TokenStream {
    emit::expand_emit_cpi(input)
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
    let item_ty = crate_aliases().resolve(&item.ty);
    let ty_value = naclac_syn::parser::rust_type_to_idl(&item_ty, &crate_constants());
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
    let value = constant_value_tokens(&item_ty, &quote::quote! { #expr });
    quote::quote! {
        #[cfg(feature = "idl-build")]
        #[doc(hidden)]
        pub fn #fn_name() -> naclac_lang::naclac_idl::IdlConstant {
            naclac_lang::naclac_idl::IdlConstant {
                name: #name.into(),
                docs: vec![#(#docs.into()),*],
                ty: naclac_lang::naclac_idl::serde_json::from_str(#ty_json)
                    .expect("naclac idl-build: generated constant type JSON must parse"),
                value: #value,
            }
        }
    }
}

/// Marker for constant declarations — `naclac-syn`'s name scanner finds a
/// constant by this attribute's bare presence on a `const` item. Under
/// `idl-build`, also generates that constant's own print function (see
/// `constant_idl_build_impl`); the attribute itself never modifies the
/// original `const` item either way.
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

/// Adds a setter per key to a constraint plugin's `Args` struct, which
/// `#[derive(Accounts)]` uses to build it from `#[account(namespace::key = ..)]`.
/// Every field must be `Option<..>`.
#[proc_macro_attribute]
pub fn constraint_args(attr: TokenStream, item: TokenStream) -> TokenStream {
    constraint_args::expand(attr, item)
}

/// Derives the necessary validation, deserialization, and constraints for an Accounts struct.
///
/// Generates zero-copy fixed array CPI properties (`[AccountMeta; N]`) regardless of the execution mode.
#[proc_macro_derive(Accounts, attributes(account, instruction, __naclac_event_cpi))]
pub fn derive_accounts(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    accounts::expand_derive_accounts(item)
}

/// Derives the correct serialization mechanism for a standalone, non-account,
/// non-event type (a struct/enum used as e.g. an instruction arg or a nested
/// field) — matching the same automatic per-mode routing `#[component]`/
/// `#[event]` already use: zero-copy mode gets `Pod`/`Zeroable` (structs) or
/// `ZcCodec` (enums) for byte-level access; Borsh mode gets real
/// `borsh::BorshSerialize`/`BorshDeserialize`.
///
/// The zero-copy struct/enum split exists because every bit pattern of a
/// fixed-size struct's bytes is a valid value (safe to declare unconditionally
/// `Pod`), but an enum's discriminant only has as many valid values as it has
/// variants — reinterpreting an out-of-range discriminant byte as the enum is
/// undefined behavior. A struct is `#[repr(C, packed)]` (alignment 1, no
/// padding); an enum is stored as its tag followed by the active variant's
/// fields, packed, through a generated `ZcCodec` whose `decode` validates the
/// tag and every nested value. `NaclacPod::naclac_try_from_bytes` returns
/// `None` for an invalid one (instruction-argument readers turn that into
/// `InvalidInstructionData`); `naclac_from_bytes` panics instead. Neither
/// blindly transmutes bytes into an enum value with no corresponding variant.
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
    if let Some(error) = type_classify::reserved_type_name_error(&ident) {
        return error.to_compile_error().into();
    }
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let is_enum = matches!(&input.data, syn::Data::Enum(_));
    // Captured before any field-padding mutation below — the IDL-visible
    // shape reflects what the developer actually wrote, not the on-chain
    // zero-copy layout's auto-added padding field.
    let original_data = input.data.clone();
    serialized_bounds::strip_max_len(&mut input.data);

    let declared_fields: Vec<&syn::Field> = match &original_data {
        syn::Data::Struct(data) => data.fields.iter().collect(),
        syn::Data::Enum(data) => data.variants.iter().flat_map(|v| v.fields.iter()).collect(),
        syn::Data::Union(_) => Vec::new(),
    };
    if let Some(error) = type_classify::zc_view_field_error(
        declared_fields.into_iter(),
        "a #[defined_type]",
        "A #[defined_type] is a fixed-size value stored in accounts and events: use a fixed-size \
         array (e.g. '[u8; 64]'). To group instruction arguments that need ZcVec/ZcString, use \
         #[instruction_args].",
    ) {
        return error.to_compile_error().into();
    }

    if let syn::Data::Enum(data) = &original_data {
        let aliases = crate_aliases();
        let variant_fields: Vec<syn::Field> = data
            .variants
            .iter()
            .flat_map(|v| v.fields.iter())
            .map(|field| syn::Field {
                ty: aliases.resolve(&field.ty),
                ..field.clone()
            })
            .collect();
        if let Some(error) = type_classify::fixed_capacity_field_error(
            variant_fields.iter(),
            "a #[defined_type] enum variant",
            "Put the FixedVec/FixedString in a #[defined_type] struct and use that struct as the \
             variant's field.",
        ) {
            return error.to_compile_error().into();
        }
    }

    // Checked here, before anything below mutates `input.attrs` (an earlier
    // version of this check ran deep inside the struct branch below, after
    // `#[repr(...)]`/`#[derive(Copy)]` had already been spliced in for the
    // zero-copy representation — its error path then echoed back that
    // already-mutated `input`, producing a cascade of unrelated `Copy`/
    // `Pod` errors instead of just this one). A user-written
    // `#[derive(Default)]` is only rejected when the macro's own
    // `Zeroable::zeroed()`-based `Default` (zero-copy mode only, further
    // below) could actually exist to conflict with it: a struct with any
    // heap-allocated field (`Vec`/`String`) can never be `Zeroable`
    // regardless of which backend is active, so the macro provides no
    // `Default` for it under any representation — rejecting the user's own
    // in that case would leave the type with none at all, even though it
    // may genuinely be needed (e.g. by a containing `#[event]`/
    // `#[instruction_args]` struct, which auto-derives `Default` on
    // itself).
    let mut zero_copy_default_conflict = proc_macro2::TokenStream::new();
    if !is_enum {
        let syn::Data::Struct(data_struct) = &original_data else {
            unreachable!("is_enum was false, so input.data must be Data::Struct");
        };
        let has_heap_field = data_struct.fields.iter().any(|f| {
            matches!(
                crate::type_classify::base_ident(&f.ty)
                    .as_ref()
                    .map(syn::Ident::to_string)
                    .as_deref(),
                Some("Vec") | Some("String")
            )
        });
        if !has_heap_field {
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
                    zero_copy_default_conflict = syn::Error::new_spanned(
                        attr,
                        "Naclac Error: in the zero-copy representation `#[defined_type]` already \
                         generates `Default` (via `Zeroable::zeroed()`) for every struct whose \
                         fields are all Pod-eligible — remove this `#[derive(Default)]`, it's \
                         redundant and, once an auto-padding field is added, incorrect (`std`'s \
                         derive is evaluated before that field exists). It is only needed under \
                         Borsh/Wincode, where no `Default` is generated.",
                    )
                    .to_compile_error();
                    break;
                }
            }
        }
    }

    // Cargo gives a proc-macro no reliable way to see the invoking crate's
    // activated features (see `component.rs`'s doc comment for the full
    // story), so both representations are always emitted below, each gated
    // by a real `#[cfg(...)]` in the output that the calling crate's own
    // compiler resolves — same as `#[component]`/`#[event]`/`#[derive(Accounts)]`.
    let zero_copy_cfg = quote::quote! { #[cfg(feature = "bytemuck")] };
    let borsh_cfg = quote::quote! { #[cfg(feature = "borsh")] };
    let wincode_cfg = quote::quote! { #[cfg(feature = "wincode")] };

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
        // Every enum gets `use_discriminant = true` unconditionally: per
        // `borsh-derive`'s own `Discriminants::new`, this reconstructs
        // Rust's real compiler-assigned value (explicit `= N` if written,
        // else previous + 1) — identical to the current, always-positional
        // behavior for an enum that never writes an explicit discriminant,
        // and only changes behavior (correctly) for one that does. A
        // struct has no discriminant at all, so this is enum-only.
        if is_enum {
            input
                .attrs
                .push(syn::parse_quote!(#[borsh(use_discriminant = true)]));
        }
        quote::quote! {
            #borsh_cfg
            #[derive(naclac_lang::prelude::BorshSerialize, naclac_lang::prelude::BorshDeserialize)]
            #[borsh(crate = "naclac_lang::prelude::borsh")]
            #input
        }
    };

    // --- Wincode variant ---
    // Each variant gets an explicit `#[wincode(tag = N)]` using the same
    // real, fully-resolved discriminant `naclac_syn::discriminator::
    // variant_discriminants` computes for the IDL and for the zero-copy
    // branch below — wincode-derive's own fallback (plain declaration
    // index) only applies when no such attribute is present, so this makes
    // Wincode respect an explicit `= N` the same way Borsh (via
    // `use_discriminant`, above) and zero-copy (native repr, below) do.
    let wincode_variant = {
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
        if let syn::Data::Enum(data_enum) = &mut input.data {
            let discriminants =
                naclac_syn::discriminator::variant_discriminants(data_enum.variants.iter());
            for (variant, discriminant) in data_enum.variants.iter_mut().zip(discriminants.iter()) {
                let tag_lit = syn::LitInt::new(&discriminant.to_string(), variant.ident.span());
                variant
                    .attrs
                    .push(syn::parse_quote!(#[wincode(tag = #tag_lit)]));
            }
        }
        quote::quote! {
            #wincode_cfg
            #[derive(naclac_lang::prelude::SchemaWrite, naclac_lang::prelude::SchemaRead)]
            #[wincode(crate = "naclac_lang::prelude::wincode")]
            #input
        }
    };

    // --- Zero-copy variant ---
    // A struct is always `#[repr(C, packed)]` (a conflicting `repr` of its own
    // is rejected). An enum gets `#[repr(u8)]` when it names no integer repr:
    // the integer is the width of its stored tag, and Rust requires one for
    // explicit discriminants on a variant with fields. The same detection
    // (`naclac_syn::discriminator::detect_enum_repr`) is used for the tag type
    // below and for the `idl-build` output, so they cannot drift apart.
    let mut zero_copy_repr_conflict = proc_macro2::TokenStream::new();
    match &input.data {
        syn::Data::Enum(_) => {
            if naclac_syn::discriminator::detect_enum_repr(&input.attrs).is_none() {
                input.attrs.push(syn::parse_quote!(#[repr(u8)]));
            }
        }
        syn::Data::Struct(_) => {
            match pod_struct_checks::repr_error(
                &input.attrs,
                "a zero-copy `#[defined_type]` struct",
            ) {
                Some(error) => zero_copy_repr_conflict = error.to_compile_error(),
                None => input.attrs.push(syn::parse_quote!(#[repr(C, packed)])),
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
        // `derive(Clone, Copy)` was already added to `input.attrs` above;
        // `ZcEnumKind` requires `Copy`.
        let syn::Data::Enum(data_enum) = &input.data else {
            unreachable!("is_enum was computed from input.data being Data::Enum")
        };
        let tag_ty: syn::Type = {
            let named = naclac_syn::discriminator::detect_enum_repr(&input.attrs);
            syn::parse_str(named.as_deref().unwrap_or("u8")).unwrap()
        };
        let codec_items = zc_enum::generate(&ident, &data_enum.variants, &tag_ty, &zero_copy_cfg);
        quote::quote! {
            #zero_copy_cfg
            #input

            #codec_items

            #zero_copy_cfg
            impl #impl_generics naclac_lang::prelude::NaclacPod for #ident #ty_generics #where_clause {
                #[inline(always)]
                fn naclac_from_bytes(data: &[u8]) -> Self {
                    // An unknown tag or invalid nested value panics instead
                    // of being undefined behaviour.
                    <Self as naclac_lang::prelude::ZcCodec>::decode(data)
                        .unwrap_or_else(|| panic!("defined_type: invalid enum discriminant in account/instruction bytes"))
                }
                #[inline(always)]
                fn naclac_try_from_bytes(data: &[u8]) -> Option<Self> {
                    <Self as naclac_lang::prelude::ZcCodec>::decode(data)
                }
                #[inline(always)]
                fn naclac_size() -> usize {
                    <Self as naclac_lang::prelude::ZcCodec>::SIZE
                }
            }
        }
    } else {
        let syn::Data::Struct(data_struct) = &input.data else {
            unreachable!("is_enum was false, so input.data must be Data::Struct")
        };
        let pod_checks = pod_struct_checks::generate(&ident, &data_struct.fields, &zero_copy_cfg);
        quote::quote! {
            #zero_copy_cfg
            #zero_copy_repr_conflict

            #zero_copy_cfg
            #input

            #pod_checks

            // SAFETY: sound only because the type is `#[repr(C, packed)]` and
            // `Copy` (both added above) and because `pod_checks` (above)
            // verifies at compile time that the struct has no padding,
            // alignment 1, and every declared field is itself Pod.
            #zero_copy_cfg
            unsafe impl #impl_generics naclac_lang::prelude::Pod for #ident #ty_generics #where_clause {}
            // SAFETY: Zeroed memory is a valid initial state for Pod types.
            #zero_copy_cfg
            unsafe impl #impl_generics naclac_lang::prelude::Zeroable for #ident #ty_generics #where_clause {}

            // Not `#[derive(Default)]` (rejected above if the caller wrote
            // it themselves) — `std`'s derive only implements `Default` for
            // arrays up to length 32, which a large fixed-size field can
            // easily exceed. `Zeroable` (already required above) has no such
            // limit, and lets every `Struct { field: v, ..Default::default() }`
            // construction site work.
            #zero_copy_cfg
            impl #impl_generics core::default::Default for #ident #ty_generics #where_clause {
                fn default() -> Self {
                    naclac_lang::prelude::bytemuck::Zeroable::zeroed()
                }
            }

            #zero_copy_cfg
            impl #impl_generics naclac_lang::prelude::ZcCodec for #ident #ty_generics #where_clause {
                const SIZE: usize = core::mem::size_of::<Self>();

                fn encode(&self, out: &mut [u8]) {
                    naclac_lang::prelude::encode_pod(self, out);
                }

                fn decode(bytes: &[u8]) -> Option<Self> {
                    naclac_lang::prelude::decode_pod(bytes)
                }
            }

            #zero_copy_cfg
            impl #impl_generics naclac_lang::prelude::NaclacPod for #ident #ty_generics #where_clause {
                #[inline(always)]
                fn naclac_from_bytes(data: &[u8]) -> Self {
                    // A short slice panics instead of reading past it.
                    naclac_lang::prelude::bytemuck::pod_read_unaligned(
                        &data[..core::mem::size_of::<Self>()],
                    )
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
        syn::Data::Struct(s) => crate::component::idl_build_impl(
            &ident,
            &syn::Generics::default(),
            &input.attrs,
            &s.fields,
            None,
        ),
        syn::Data::Enum(e) => idl_build_impl_enum(&ident, &e.variants, &input.attrs),
        syn::Data::Union(_) => quote::quote! {},
    };

    let zero_copy_default_conflict = if zero_copy_default_conflict.is_empty() {
        quote::quote! {}
    } else {
        quote::quote! { #zero_copy_cfg #zero_copy_default_conflict }
    };

    let discriminant_range_conflict = match &original_data {
        syn::Data::Enum(data_enum) => {
            let discriminants =
                naclac_syn::discriminator::variant_discriminants(data_enum.variants.iter());
            if discriminants.iter().all(|d| (0..=255).contains(d)) {
                quote::quote! {}
            } else {
                let message = format!(
                    "Naclac Error: `#[defined_type]` enum `{ident}` has a discriminant outside \
                     0..=255. Borsh and Wincode encode an enum tag as a single unsigned byte; \
                     use discriminants in that range, or the zero-copy representation."
                );
                quote::quote! {
                    #borsh_cfg
                    ::core::compile_error!(#message);
                    #wincode_cfg
                    ::core::compile_error!(#message);
                }
            }
        }
        _ => quote::quote! {},
    };

    let bounds_cfg = quote::quote! {
        #[cfg(any(feature = "borsh", feature = "wincode"))]
    };
    let bounds_impl = match serialized_bounds::generate(
        &syn::DeriveInput {
            data: original_data.clone(),
            ..input.clone()
        },
        &bounds_cfg,
    ) {
        Ok(tokens) => tokens,
        Err(err) => err.to_compile_error(),
    };

    quote::quote! {
        #discriminant_range_conflict
        #zero_copy_default_conflict
        #bounds_impl
        #borsh_variant
        #wincode_variant
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
    let enum_docs = naclac_syn::parser::extract_docs(attrs);
    let repr =
        naclac_syn::discriminator::detect_enum_repr(attrs).unwrap_or_else(|| "u8".to_string());
    let repr_tokens = quote::quote! { Some(#repr.to_string()) };

    let discriminants = naclac_syn::discriminator::variant_discriminants(variants.iter());
    let constants = crate_constants();
    let aliases = crate_aliases();
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
                    let fname =
                        naclac_syn::parser::idl_name(&f.ident.as_ref().unwrap().to_string());
                    let field_docs = naclac_syn::parser::extract_docs(&f.attrs);
                    let field_ty = aliases.resolve(&f.ty);
                    let ty_value = naclac_syn::parser::rust_type_to_idl(&field_ty, &constants);
                    let Ok(ty_json) = serde_json::to_string(&ty_value) else {
                        continue;
                    };
                    field_ts_list.push(quote::quote! {
                        naclac_lang::naclac_idl::IdlField {
                            name: #fname.into(),
                            docs: vec![#(#field_docs.into()),*],
                            ty: naclac_lang::naclac_idl::serde_json::from_str(#ty_json)
                                .expect("naclac idl-build: generated field type JSON must parse"),
                            slots: None,
                        }
                    });
                    if let Some(defined_ty) = naclac_syn::parser::defined_type_leaf(&field_ty) {
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
                    let field_ty = aliases.resolve(&f.ty);
                    let ty_value = naclac_syn::parser::rust_type_to_idl(&field_ty, &constants);
                    let Ok(ty_json) = serde_json::to_string(&ty_value) else {
                        continue;
                    };
                    ty_ts_list.push(quote::quote! {
                        naclac_lang::naclac_idl::serde_json::from_str(#ty_json)
                            .expect("naclac idl-build: generated field type JSON must parse")
                    });
                    if let Some(defined_ty) = naclac_syn::parser::defined_type_leaf(&field_ty) {
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
                    docs: vec![#(#enum_docs.into()),*],
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
/// (`ZcString`/`ZcVec<T>`/`Vec<T>`/`String`) field is prefixed with a
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
        // `Option<T>` decodes correctly through the same `NaclacPod` call as
        // a true `Fixed` field — see `DynamicKind::OptionPod`'s doc comment.
        DynamicKind::Fixed | DynamicKind::OptionPod => quote::quote! {
            let #field_name = {
                let __sz = <#field_ty as naclac_lang::prelude::NaclacPod>::naclac_size();
                if data.len() < *offset + __sz {
                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                }
                let __val = <#field_ty as naclac_lang::prelude::NaclacPod>::naclac_try_from_bytes(
                    &data[*offset..*offset + __sz]
                )
                .ok_or_else(|| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?;
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
                    let __val = <naclac_lang::prelude::ZcVec<#inner>>::from_bytes(&data[*offset..*offset + __len])?;
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
        DynamicKind::HeapVec => {
            // `Vec<Vec<u8>>`/`Vec<String>`: the prefix is an element count and each element
            // carries its own length, so the shared reader advances the offset itself.
            if type_classify::first_generic_type(field_ty).is_some_and(|inner| {
                type_classify::is_exactly(inner, "Vec")
                    || type_classify::is_exactly(inner, "String")
            }) {
                return quote::quote! {
                    let #field_name = {
                        if data.len() < *offset + 4 {
                            return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                        }
                        let mut __count_bytes = [0u8; 4];
                        __count_bytes.copy_from_slice(&data[*offset..*offset + 4]);
                        let __count = u32::from_le_bytes(__count_bytes) as usize;
                        *offset += 4;
                        naclac_lang::prelude::read_length_prefixed_items(data, offset, __count)?
                    };
                };
            }
            let elements = match type_classify::first_generic_type(field_ty) {
                Some(inner) if type_classify::is_exactly(inner, "u8") => quote::quote! {
                    data[*offset..*offset + __len].to_vec()
                },
                Some(inner) => quote::quote! {{
                    let __sz = <#inner as naclac_lang::prelude::NaclacPod>::naclac_size();
                    if __sz == 0 || __len % __sz != 0 {
                        return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                    }
                    let mut __items = naclac_lang::prelude::Vec::with_capacity(__len / __sz);
                    let mut __at = *offset;
                    while __at < *offset + __len {
                        __items.push(
                            <#inner as naclac_lang::prelude::NaclacPod>::naclac_try_from_bytes(
                                &data[__at..__at + __sz],
                            )
                            .ok_or_else(|| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?,
                        );
                        __at += __sz;
                    }
                    __items
                }},
                None => quote::quote! {
                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))
                },
            };
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
                    let __val = #elements;
                    *offset += __len;
                    __val
                };
            }
        }
    }
}

/// Proc macro attribute for grouping instruction arguments.
/// Zero-copy (`bytemuck`), fixed-size struct: `repr(C)` with explicit hidden padding fields, `Clone`, `Copy`,
/// `Pod`, `Zeroable`, `Default` and `NaclacPod`. Borsh and Wincode, fixed-size struct: `Clone` plus the Borsh or
/// Wincode derives only. A struct with a `String`/`Vec`/`Option`/`ZcVec`/`ZcString` field is read field by field
/// through `NaclacArgs` in zero-copy mode.
#[proc_macro_attribute]
pub fn instruction_args(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    if let Some(err) = reject_nonempty_attr(&attr, "instruction_args") {
        return err;
    }
    let mut input = syn::parse_macro_input!(item as syn::DeriveInput);

    // `#[allow_heap]` on a field silences the heap-collection warning below;
    // it is not a real attribute, so it is removed from the emitted struct.
    let mut allow_heap_fields = std::collections::HashSet::<String>::new();
    if let syn::Data::Struct(data_struct) = &mut input.data {
        if let syn::Fields::Named(fields_named) = &mut data_struct.fields {
            for field in fields_named.named.iter_mut() {
                let had_allow_heap = field.attrs.iter().any(|a| a.path().is_ident("allow_heap"));
                field.attrs.retain(|a| !a.path().is_ident("allow_heap"));
                if let (true, Some(name)) = (had_allow_heap, &field.ident) {
                    allow_heap_fields.insert(name.to_string());
                }
            }
        }
    }

    let declared_input = input.clone();
    let view_lifetime = type_classify::thread_view_lifetime(&mut input);

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
    let zero_copy_cfg = quote::quote! { #[cfg(feature = "bytemuck")] };
    let not_zero_copy_cfg = quote::quote! { #[cfg(not(feature = "bytemuck"))] };

    // In zero-copy mode a fixed-field struct gets a generated `Default` impl
    // (below, via `Zeroable::zeroed()`) — a user-written `#[derive(Default)]`
    // would conflict with it, so reject it the same way `defined_type` does.
    // Borsh and Wincode modes generate no `Default`, so the derive stays valid
    // there and the error is gated to the zero-copy cfg.
    let mut default_derive_error = quote::quote! {};
    if !has_dynamic_field {
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
                let error = syn::Error::new_spanned(
                    attr,
                    "Naclac Error: `#[instruction_args]` already generates `Default` (via \
                     `Zeroable::zeroed()`) for a fixed-field struct in zero-copy mode — remove \
                     this `#[derive(Default)]`, it's redundant.",
                )
                .to_compile_error();
                default_derive_error = quote::quote! { #zero_copy_cfg #error };
                break;
            }
        }
    }
    // `ZcVec<T>`/`ZcString` are zero-copy *views* (pointer +
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
            "Naclac Error: field '{}' uses a zero-copy-only type ('ZcVec<T>'/'ZcString') \
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

    let (wincode_derive_attr, wincode_error) = if let Some(field) = zc_field {
        let field_name = field.ident.as_ref().expect("named field");
        let msg = format!(
            "Naclac Error: field '{}' uses a zero-copy-only type ('ZcVec<T>'/'ZcString') \
             in a Wincode-mode #[instruction_args]. These are views (pointer + length) into the \
             current instruction's byte buffer with no bytes of their own to serialize — use \
             'Vec<T>'/'String' instead.",
            field_name
        );
        (
            quote::quote! {},
            quote::quote! { #[cfg(feature = "wincode")] compile_error!(#msg); },
        )
    } else {
        (
            quote::quote! {
                #[cfg_attr(feature = "wincode", derive(naclac_lang::prelude::SchemaWrite, naclac_lang::prelude::SchemaRead))]
                #[cfg_attr(feature = "wincode", wincode(crate = "naclac_lang::prelude::wincode"))]
            },
            quote::quote! {},
        )
    };
    // `#[instruction_args]` structs are directly usable as an instruction
    // arg's own type, so `#[program]`'s idl-build assembler chases them the
    // same way it chases any other non-primitive arg type
    // (`naclac_syn::parser::defined_type_leaf`) — without this, that chase
    // would generate `<Self as NaclacIdlBuild>::create_type()` for a type
    // that never implements the trait, a hard compile error the moment a
    // real program used `#[instruction_args]` under `idl-build`. Reuses
    // `component.rs`'s own `idl_build_impl` directly rather than
    // duplicating it — same named-fields shape, same per-field type mapping.
    let syn::Data::Struct(declared_struct) = &declared_input.data else {
        unreachable!("checked above: input is a struct")
    };
    let idl_build_impl_tokens = component::idl_build_impl(
        ident,
        &input.generics,
        &input.attrs,
        &declared_struct.fields,
        None,
    );

    if !has_dynamic_field {
        // Zero-copy mode reads this struct as one `repr(C, packed)` block
        // (alignment 1, no padding), which is what makes the `Pod` impl below
        // true. Borsh and Wincode modes encode field by field and keep the
        // struct as written.
        let pod_checks = pod_struct_checks::generate(ident, &data_struct.fields, &zero_copy_cfg);

        let expanded = quote::quote! {
            #default_derive_error

            #zero_copy_cfg
            #[repr(C, packed)]
            #[derive(Clone, Copy)]
            #input

            #pod_checks

            #zero_copy_cfg
            unsafe impl naclac_lang::prelude::Pod for #ident #ty_generics #where_clause {}
            #zero_copy_cfg
            unsafe impl naclac_lang::prelude::Zeroable for #ident #ty_generics #where_clause {}

            // `Zeroable` rather than `#[derive(Default)]` — consistent with
            // `defined_type`'s own `Default` impl, and it keeps every
            // `Struct { field: v, ..Default::default() }` call site working.
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
                    // A short slice panics instead of reading past it.
                    naclac_lang::prelude::bytemuck::pod_read_unaligned(
                        &data[..core::mem::size_of::<Self>()],
                    )
                }
                #[inline(always)]
                fn naclac_size() -> usize {
                    core::mem::size_of::<Self>()
                }
            }

            #not_zero_copy_cfg
            #[derive(Clone)]
            #borsh_derive_attr
            #wincode_derive_attr
            #input

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
        // A heap `Vec<T>`/`String` field is valid but allocates and copies on
        // every deserialize, so it gets the same silenceable warning a plain
        // `Vec<T>`/`String` instruction argument gets.
        let field_reads: Vec<proc_macro2::TokenStream> = fields_named
            .named
            .iter()
            .map(|f| {
                let name = f.ident.as_ref().expect("named field");
                let read = field_deserialize_stmt(name, &f.ty);
                let heap_shape = match type_classify::classify_dynamic(&f.ty) {
                    type_classify::DynamicKind::HeapVec => Some(("Vec<T>", "ZcVec<T>")),
                    type_classify::DynamicKind::HeapString => Some(("String", "ZcString")),
                    _ => None,
                };
                match heap_shape {
                    Some((found, suggested)) if !allow_heap_fields.contains(&name.to_string()) => {
                        let warning = heap_collection_warning(&name.to_string(), found, suggested);
                        quote::quote! { #warning #read }
                    }
                    _ => read,
                }
            })
            .collect();

        let args_lifetime = view_lifetime.clone().unwrap_or_else(|| {
            syn::Lifetime::new("'__naclac_data", proc_macro2::Span::call_site())
        });
        let mut args_generics = input.generics.clone();
        if view_lifetime.is_none() {
            args_generics.params.insert(
                0,
                syn::GenericParam::Lifetime(syn::LifetimeParam::new(args_lifetime.clone())),
            );
        }
        let (args_impl_generics, _, _) = args_generics.split_for_impl();

        quote::quote! {
            #zero_copy_cfg
            impl #args_impl_generics naclac_lang::prelude::NaclacArgs<#args_lifetime> for #ident #ty_generics #where_clause {
                fn naclac_deserialize(
                    data: &#args_lifetime [u8],
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
        #wincode_error

        #[derive(Clone)]
        #borsh_derive_attr
        #wincode_derive_attr
        #input

        #naclac_args_impl
        #idl_build_impl_tokens
    };
    expanded.into()
}
