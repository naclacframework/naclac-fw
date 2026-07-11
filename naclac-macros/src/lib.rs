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

use proc_macro::TokenStream;

fn check_banned_functions(tokens: &TokenStream) -> Option<TokenStream> {
    let token_str = tokens.to_string();
    if token_str.contains("find_program_address") || token_str.contains("create_program_address") {
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
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let toml_path = std::path::Path::new(&manifest_dir).join("Cargo.toml");
        if let Ok(content) = std::fs::read_to_string(&toml_path) {
            for line in content.lines() {
                let clean: String = line.chars().filter(|c| !c.is_whitespace()).collect();
                if clean.contains("naclac-lang") && clean.contains(feature) {
                    return true;
                }
            }
        }
    }
    false
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
    if let Some(err) = check_banned_functions(&item) {
        return err;
    }
    instruction::expand(attr, item)
}

/// The main entrypoint macro for a Naclac smart contract.
///
/// This generates the overarching instruction dispatcher and handles Cross-Program
/// Invocation (CPI) module generation. It supports `zero_copy` configuration natively.
#[proc_macro_attribute]
pub fn program(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Some(err) = check_banned_functions(&item) {
        return err;
    }
    program::expand(attr, item)
}

/// Generates standardized custom error codes for the program.
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

/// Stub for CPI attributes (ignored, acts as a marker).
#[proc_macro_attribute]
pub fn cpi(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Stub for constant declarations (ignored, acts as a marker).
#[proc_macro_attribute]
pub fn constant(_attr: TokenStream, item: TokenStream) -> TokenStream {
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
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut input = syn::parse_macro_input!(item as syn::DeriveInput);
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Enums default to a compiler-chosen discriminant layout, so auto-add
    // #[repr(u8)] when the caller hasn't picked one explicitly. Structs are
    // NOT touched here — the caller is responsible for giving the struct a
    // deterministic layout (e.g. #[repr(C)]) themselves; see the SAFETY note below.
    if let syn::Data::Enum(_) = &input.data {
        let has_repr = input.attrs.iter().any(|attr| attr.path().is_ident("repr"));
        if !has_repr {
            input.attrs.push(syn::parse_quote!(#[repr(u8)]));
        }
    }

    let expanded = quote::quote! {
        #input

        // SAFETY: sound only if the annotated type already has a deterministic
        // layout — #[repr(u8)] for enums (auto-added above when missing) or
        // #[repr(C)] for structs (NOT auto-added; the caller must annotate this themselves).
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

/// Proc macro attribute for grouping instruction arguments.
/// Implements repr(C), Clone, Copy, Pod, Zeroable, NaclacPod, and optionally Borsh serialize/deserialize.
#[proc_macro_attribute]
pub fn instruction_args(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
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
    expanded.into()
}
