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
mod component;
mod error_code;
mod event;
mod instruction;
mod program;
mod system;
mod type_classify;

use proc_macro::TokenStream;

/// Walks an AST looking for a real call (function call or method call) to
/// `find_program_address`/`create_program_address` — matched by the exact
/// resolved segment/method ident, not a substring anywhere in the item's
/// text, so a helper merely *named* `log_find_program_address_ban` or a
/// local variable `create_program_address_msg` can't trip it.
#[derive(Default)]
struct BannedFnCallVisitor {
    found: bool,
}

impl<'ast> syn::visit::Visit<'ast> for BannedFnCallVisitor {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(p) = &*node.func {
            if let Some(seg) = p.path.segments.last() {
                if seg.ident == "find_program_address" || seg.ident == "create_program_address" {
                    self.found = true;
                }
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "find_program_address" || node.method == "create_program_address" {
            self.found = true;
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

fn check_banned_functions(tokens: &TokenStream) -> Option<TokenStream> {
    let Ok(item) = syn::parse::<syn::Item>(tokens.clone()) else {
        return None;
    };
    let mut visitor = BannedFnCallVisitor::default();
    syn::visit::visit_item(&mut visitor, &item);
    if visitor.found {
        let error_msg = "Naclac Error: 'find_program_address' and 'create_program_address' are not supported in Naclac. \
                         To optimize Compute Units (CUs), Naclac strictly bans on-chain PDA derivation searches. \
                         Please pass the bump seed from the client and validate using the hash-and-compare optimization \
                         (e.g., using `#[account(seeds = [...], bump = my_bump)]` or `#[account(seeds = [...], bump)]`).";
        return Some(
            syn::Error::new(proc_macro2::Span::call_site(), error_msg)
                .to_compile_error()
                .into(),
        );
    }
    None
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

/// Determines whether the crate *currently being compiled* (i.e. the crate invoking
/// this proc-macro right now) has requested a given naclac-lang feature (e.g.
/// `"borsh"`, `"pinocchio"`).
///
/// This deliberately does NOT use `cfg!(feature = ...)` — a `cfg!()` written in this
/// crate's own source reflects `naclac-macros`'s own feature set at the time
/// `naclac-macros` itself was compiled, not the feature set of whichever downstream
/// crate happens to be invoking the macro during its own build. Since a proc-macro
/// runs in-process as part of the invoking crate's `rustc` invocation, it can instead
/// inspect that specific build directly: `CARGO_FEATURE_<NAME>` is set by Cargo for
/// features the invoking crate declares directly on itself. For features requested
/// only transitively (e.g. `naclac-lang = { features = ["borsh"] }` in the invoking
/// crate's own `Cargo.toml`, rather than a `borsh` feature of its own), we fall back
/// to scanning that crate's manifest text directly.
pub(crate) fn caller_has_feature(feature: &str) -> bool {
    let env_name = format!("CARGO_FEATURE_{}", feature.to_uppercase().replace('-', "_"));
    if std::env::var(&env_name).is_ok() {
        return true;
    }
    let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        return false;
    };
    let toml_path = std::path::Path::new(&manifest_dir).join("Cargo.toml");
    let Ok(content) = std::fs::read_to_string(&toml_path) else {
        return false;
    };
    let Ok(manifest) = toml::from_str::<toml::Value>(&content) else {
        return false;
    };
    manifest
        .get("dependencies")
        .and_then(|deps| deps.get("naclac-lang"))
        .and_then(|dep| dep.get("features"))
        .and_then(|features| features.as_array())
        .is_some_and(|features| features.iter().any(|f| f.as_str() == Some(feature)))
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

/// Attribute for annotating an instruction logic function.
#[proc_macro_attribute]
pub fn instruction(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    if let Some(err) = reject_nonempty_attr(&attr, "instruction") {
        return err;
    }
    if let Some(err) = check_banned_functions(&item) {
        return err;
    }
    instruction::expand(attr, item)
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
    if let Some(err) = check_banned_functions(&item) {
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

/// Stub for constant declarations (ignored, acts as a marker) — `naclac-syn`'s
/// IDL/offchain-generator scan looks for this attribute's bare presence on a
/// `const` item; the macro itself does nothing beyond that.
#[proc_macro_attribute]
pub fn constant(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Some(err) = reject_nonempty_attr(&attr, "constant") {
        return err;
    }
    item
}

/// Derives the necessary validation, deserialization, and constraints for an Accounts struct.
///
/// Generates zero-copy fixed array CPI properties (`[AccountMeta; N]`) regardless of the execution mode.
#[proc_macro_derive(Accounts, attributes(account, instruction))]
pub fn derive_accounts(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    if let Some(err) = check_banned_functions(&item) {
        return err;
    }
    accounts::expand_derive_accounts(item)
}

/// Internal derivation for legacy Borsh serialization fallback.
#[proc_macro_derive(NaclacSerialize)]
pub fn derive_naclac_serialize(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let body = match &input.data {
        syn::Data::Struct(data_struct) => {
            let mut field_serializations = Vec::new();
            for field in &data_struct.fields {
                let field_ident = &field.ident;
                field_serializations.push(quote::quote! {
                    naclac_lang::prelude::borsh::BorshSerialize::serialize(&self.#field_ident, writer)?;
                });
            }
            quote::quote! { #(#field_serializations)* }
        }
        syn::Data::Enum(data_enum) => {
            let mut arms = Vec::new();
            for (i, variant) in data_enum.variants.iter().enumerate() {
                let variant_ident = &variant.ident;
                let idx = i as u8;
                arms.push(quote::quote! {
                    #ident::#variant_ident => {
                        writer.write_all(&[#idx])?;
                    }
                });
            }
            quote::quote! {
                match self {
                    #(#arms)*
                }
            }
        }
        _ => panic!("NaclacSerialize only supports Structs and Enums"),
    };

    let expanded = quote::quote! {
        impl #impl_generics naclac_lang::prelude::borsh::BorshSerialize for #ident #ty_generics #where_clause {
            fn serialize<W: naclac_lang::prelude::borsh::io::Write>(&self, writer: &mut W) -> naclac_lang::prelude::borsh::io::Result<()> {
                #body
                Ok(())
            }
        }
    };
    expanded.into()
}

/// Derives the necessary `Pod` and `Zeroable` traits for safe byte-level casting.
///
/// Critical for achieving `no_std` zero-copy performance without data corruption.
#[proc_macro_attribute]
pub fn naclac_pod(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    if let Some(err) = reject_nonempty_attr(&attr, "naclac_pod") {
        return err;
    }
    let mut input = syn::parse_macro_input!(item as syn::DeriveInput);
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Auto-add a deterministic layout repr when the caller hasn't picked one
    // explicitly: #[repr(u8)] for enums, #[repr(C)] for structs.
    let has_repr = input.attrs.iter().any(|attr| attr.path().is_ident("repr"));
    if !has_repr {
        match &input.data {
            syn::Data::Enum(_) => input.attrs.push(syn::parse_quote!(#[repr(u8)])),
            syn::Data::Struct(_) => input.attrs.push(syn::parse_quote!(#[repr(C)])),
            syn::Data::Union(_) => {}
        }
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

    let expanded = quote::quote! {
        #input

        // SAFETY: sound only because the annotated type has a deterministic
        // layout (#[repr(u8)]/#[repr(C)]) and is Copy, both auto-added above
        // when the caller hasn't already provided them.
        unsafe impl #impl_generics naclac_lang::prelude::Pod for #ident #ty_generics #where_clause {}
        // SAFETY: Zeroed memory is a valid initial state for Pod types.
        unsafe impl #impl_generics naclac_lang::prelude::Zeroable for #ident #ty_generics #where_clause {}

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
    };
    expanded.into()
}

/// Internal derivation for legacy Borsh deserialization fallback.
#[proc_macro_derive(NaclacDeserialize)]
pub fn derive_naclac_deserialize(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let body = match &input.data {
        syn::Data::Struct(data_struct) => {
            let mut field_deserializations = Vec::new();
            let mut field_names = Vec::new();
            for field in &data_struct.fields {
                let field_ident = &field.ident;
                field_names.push(field_ident);
                field_deserializations.push(quote::quote! {
                    let #field_ident = naclac_lang::prelude::borsh::BorshDeserialize::deserialize_reader(reader)?;
                });
            }
            quote::quote! {
                #(#field_deserializations)*
                Ok(Self { #(#field_names),* })
            }
        }
        syn::Data::Enum(data_enum) => {
            let mut arms = Vec::new();
            for (i, variant) in data_enum.variants.iter().enumerate() {
                let variant_ident = &variant.ident;
                let idx = i as u8;
                arms.push(quote::quote! {
                    #idx => Ok(#ident::#variant_ident),
                });
            }
            quote::quote! {
                let mut tag = [0u8; 1];
                reader.read_exact(&mut tag)?;
                match tag[0] {
                    #(#arms)*
                    _ => Err(naclac_lang::prelude::borsh::io::Error::new(
                        naclac_lang::prelude::borsh::io::ErrorKind::InvalidData,
                        "",
                    )),
                }
            }
        }
        _ => panic!("NaclacDeserialize only supports Structs and Enums"),
    };

    let expanded = quote::quote! {
        impl #impl_generics naclac_lang::prelude::borsh::BorshDeserialize for #ident #ty_generics #where_clause {
            fn deserialize_reader<R: naclac_lang::prelude::borsh::io::Read>(reader: &mut R) -> naclac_lang::prelude::borsh::io::Result<Self> {
                #body
            }
        }
    };
    expanded.into()
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

    let has_borsh = caller_has_feature("borsh");

    // NOTE: This must be *real*, field-by-field Borsh encoding (via the actual
    // `borsh` derive macros), not a raw `size_of::<Self>()` memory copy. A raw
    // memcpy silently includes any repr(C) alignment padding, which diverges
    // from the client SDK's real (tightly-packed) Borsh encoding the moment a
    // struct's field order forces the compiler to insert padding — e.g. a u64
    // following a run of u8 fields. That divergence is a wire-format mismatch,
    // not a compile-time-detectable one, so it only surfaces as a runtime
    // `InvalidInstructionData` deserialization failure on-chain.
    let borsh_derive_attr = if has_borsh {
        quote::quote! {
            #[derive(naclac_lang::prelude::BorshSerialize, naclac_lang::prelude::BorshDeserialize)]
            #[borsh(crate = "naclac_lang::prelude::borsh")]
        }
    } else {
        quote::quote! {}
    };

    // A field is only safe to treat as `Pod` (raw `size_of`-based byte cast,
    // both ways) when its own bytes fully determine its value with no
    // pointer/length indirection — `ZcString`/`Span<T>` are zero-copy
    // *views* (pointer + length) into the current instruction's byte buffer,
    // and casting arbitrary wire bytes directly onto that representation
    // would materialize an attacker-controlled raw pointer (`Span::ptr`)
    // later dereferenced by `.as_str()`/`.as_bytes()` — undefined behavior,
    // not just a wrong value. `Vec<T>`/`String` fields are unsafe for the
    // same reason `derive(Copy)` already rejects them: they own a heap
    // allocation, not an in-place byte pattern.
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
        };
        return expanded.into();
    }

    // Zero-copy mode only — in Borsh mode, `program.rs`'s dispatch reads this
    // arg via `BorshDeserialize::deserialize` directly (never `NaclacArgs`),
    // and the derived `BorshSerialize`/`BorshDeserialize` above already
    // handles `String`/`Vec<T>` fields natively.
    let naclac_args_impl = if has_borsh {
        quote::quote! {}
    } else {
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
        #[derive(Clone)]
        #borsh_derive_attr
        #input

        #naclac_args_impl
    };
    expanded.into()
}
