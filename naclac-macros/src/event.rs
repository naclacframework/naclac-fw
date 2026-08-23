//! # Event Macro Logic
//!
//! Handles the expansion of the `#[event]` macro, generating efficient event logging logic.
//! It computes the 8-byte discriminator at compile time and emits events using either
//! highly optimized zero-copy slicing (`bytemuck`), a single-allocation dynamic buffer
//! (`#[event(alloc)]`, for events needing `Vec`/`String`/`Option`), or standard Borsh
//! serialization.

use proc_macro::TokenStream;
use quote::quote;
use sha2::{Digest, Sha256};
use syn::{parse_macro_input, GenericArgument, ItemStruct, PathArguments, Type};

/// How a field's bytes get written into an `#[event(alloc)]` buffer.
enum FieldKind {
    /// `String` — u32 LE length prefix + raw UTF-8 bytes.
    String,
    /// `Vec<T>` — u32 LE length prefix + each element's `bytemuck::bytes_of`.
    Vec(Type),
    /// `Option<T>` — 1-byte tag + `bytemuck::bytes_of` when `Some`.
    Option(Type),
    /// Anything else — assumed `Pod`, written via `bytemuck::bytes_of`.
    Fixed,
}

/// Structural (not string-matching) detection of `String`/`Vec<T>`/`Option<T>` —
/// reads the real parsed type via `syn`, so it can never misclassify a type
/// the way a name-based heuristic could.
fn classify_field_type(ty: &Type) -> FieldKind {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            let name = seg.ident.to_string();
            if name == "String" {
                return FieldKind::String;
            }
            if name == "Vec" || name == "Option" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return if name == "Vec" {
                            FieldKind::Vec(inner.clone())
                        } else {
                            FieldKind::Option(inner.clone())
                        };
                    }
                }
            }
        }
    }
    FieldKind::Fixed
}

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as ItemStruct);
    let struct_name = &ast.ident;
    let vis = &ast.vis;

    // Validate that no fields use the Pubkey type
    for field in ast.fields.iter() {
        let ty = &field.ty;
        if crate::type_classify::is_deprecated_pubkey(ty) {
            return syn::Error::new_spanned(
                ty,
                "Naclac Error: 'Pubkey' has been deprecated in favor of 'Address' in Solana v3. Please replace it with 'Address'."
            )
            .to_compile_error()
            .into();
        }
    }

    let discriminator_preimage = format!("event:{}", struct_name);
    let mut hasher = Sha256::new();
    hasher.update(discriminator_preimage.as_bytes());
    let result = hasher.finalize();

    let b0 = result[0];
    let b1 = result[1];
    let b2 = result[2];
    let b3 = result[3];
    let b4 = result[4];
    let b5 = result[5];
    let b6 = result[6];
    let b7 = result[7];

    // Resolved now, at macro-expansion time, via `caller_has_feature` (reads
    // the calling crate's own Cargo.toml) rather than emitting a runtime
    // `#[cfg(feature = "borsh")]` into the generated code — a raw `cfg` like
    // that would check the *calling* crate's own `borsh` feature, which none
    // of naclac's own `_borsh` program crates actually declare (they only
    // enable `naclac-lang`'s `borsh` feature), so it always resolved false.
    let is_zero_copy =
        crate::caller_has_feature("pinocchio") || !crate::caller_has_feature("borsh");

    // `#[event]` takes no argument; `#[event(alloc)]` takes exactly the bare
    // `alloc` identifier and nothing else. Anything other than those two
    // shapes is a real error, not a silent no-op.
    let use_alloc = if attr.is_empty() {
        false
    } else {
        match syn::parse::<syn::Ident>(attr) {
            Ok(ident) if ident == "alloc" => true,
            Ok(ident) => {
                return syn::Error::new(
                    ident.span(),
                    format!(
                        "Naclac Error: unknown argument '{ident}' for #[event] — the only valid argument is 'alloc'."
                    ),
                )
                .to_compile_error()
                .into();
            }
            Err(_) => {
                return syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "Naclac Error: #[event] takes no argument, or exactly 'alloc' — e.g. #[event(alloc)].",
                )
                .to_compile_error()
                .into();
            }
        }
    };

    if use_alloc {
        return expand_alloc(
            &ast,
            struct_name,
            [b0, b1, b2, b3, b4, b5, b6, b7],
            is_zero_copy,
        );
    }

    // --- Size/alignment for automatic padding, computed by the COMPILER via
    // size_of/align_of rather than a proc-macro-time string-matching guess.
    // A string heuristic can only ever recognize types it was told about by
    // name (primitives, Opt<T>, literal [u8; N]) — any custom struct field
    // (including a caller's own newtype wrapper) would silently fall back to
    // a wrong guess, corrupting the padding computation. size_of/align_of are
    // always correct for any Sized type, resolved after macro expansion once
    // the compiler has full type information.
    let mut fields_list = quote! {};
    let mut size_sum = quote! { 0usize };
    let mut align_max = quote! { 1usize };
    for field in ast.fields.iter() {
        let f_vis = &field.vis;
        let f_ident = &field.ident;
        let f_ty = &field.ty;
        fields_list = quote! { #fields_list #f_vis #f_ident: #f_ty, };
        size_sum = quote! { #size_sum + core::mem::size_of::<#f_ty>() };
        align_max = quote! {{
            let __a = #align_max;
            let __b = core::mem::align_of::<#f_ty>();
            if __a > __b { __a } else { __b }
        }};
    }

    let padding_const_name = syn::Ident::new(
        &format!("__{}_PADDING_SIZE", struct_name),
        struct_name.span(),
    );

    let expanded = if is_zero_copy {
        quote! {
            #[allow(non_upper_case_globals)]
            const #padding_const_name: usize = {
                let __total: usize = #size_sum;
                let __align: usize = #align_max;
                let __rem = __total % __align;
                if __rem == 0 { 0 } else { __align - __rem }
            };

            #[cfg_attr(feature = "debug-mode", derive(Debug))]
            #[derive(Clone, Copy, naclac_lang::prelude::Pod, naclac_lang::prelude::Zeroable)]
            #[bytemuck(crate = "naclac_lang::bytemuck")]
            #[repr(C)]
            #vis struct #struct_name {
                #fields_list
                pub _padding: [u8; #padding_const_name],
            }

            // Not `#[derive(Default)]`: the standard library only implements
            // `Default` for arrays up to length 32, so a `[u8; N]`-shaped
            // field with N > 32 would break the derive even though the
            // struct is perfectly valid to zero-initialize. `Zeroable`
            // (already required above) has no such limit.
            impl Default for #struct_name {
                fn default() -> Self {
                    naclac_lang::prelude::bytemuck::Zeroable::zeroed()
                }
            }

            impl #struct_name {
                pub fn emit(&self) {
                    let discriminator = [#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7];
                    let data = naclac_lang::prelude::bytemuck::bytes_of(self);
                    naclac_lang::prelude::sol_log_data(&[&discriminator, data]);
                }
            }
        }
    } else {
        quote! {
            #[derive(Clone, naclac_lang::prelude::BorshSerialize, Default)]
            #[cfg_attr(feature = "debug-mode", derive(Debug))]
            #[borsh(crate = "naclac_lang::prelude::borsh")]
            #ast

            impl #struct_name {
                pub fn emit(&self) {
                    naclac_lang::event::emit_event(self, [#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7]);
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// `#[event(alloc)]` — for events needing `Vec`/`String`/`Option` fields.
/// Only meaningful under a zero-copy backend (Pinocchio, or Solana Program +
/// Zero-Copy): under Borsh mode, plain `#[event]` already serializes these
/// types natively via real Borsh, so `alloc` mode is redundant there — kept
/// as a hard compile error rather than silently falling back, so a caller
/// never mistakenly believes they got the single-allocation buffer path.
///
/// No `repr(C)`/`Pod`/`Zeroable`/padding at all: the struct is declared with
/// its real field types, and `emit()` computes the exact total byte length
/// up front (constant field sizes plus a runtime `.len()` for each dynamic
/// field), allocates the buffer once via `Vec::with_capacity`, then writes
/// every field sequentially — no reallocation ever happens because the
/// buffer is already correctly sized before the first byte is written.
fn expand_alloc(
    ast: &ItemStruct,
    struct_name: &syn::Ident,
    disc: [u8; 8],
    is_zero_copy: bool,
) -> TokenStream {
    if !is_zero_copy {
        return syn::Error::new_spanned(
            ast,
            "Naclac Error: #[event(alloc)] is only for zero-copy backends (Pinocchio, or \
             Solana Program + Zero-Copy). Under Borsh mode, plain #[event] already supports \
             Vec/String/Option natively via real Borsh, so #[event(alloc)] is unnecessary \
             there — use plain #[event] instead.",
        )
        .to_compile_error()
        .into();
    }

    let [b0, b1, b2, b3, b4, b5, b6, b7] = disc;

    let mut size_expr = quote! { 0usize };
    let mut write_stmts = quote! {};

    for field in ast.fields.iter() {
        let f_ident = &field.ident;
        let f_ty = &field.ty;

        match classify_field_type(f_ty) {
            FieldKind::String => {
                size_expr = quote! { #size_expr + 4 + self.#f_ident.len() };
                write_stmts = quote! {
                    #write_stmts
                    __buf.extend_from_slice(&(self.#f_ident.len() as u32).to_le_bytes());
                    __buf.extend_from_slice(self.#f_ident.as_bytes());
                };
            }
            FieldKind::Vec(inner_ty) => {
                size_expr = quote! {
                    #size_expr + 4 + self.#f_ident.len() * core::mem::size_of::<#inner_ty>()
                };
                write_stmts = quote! {
                    #write_stmts
                    __buf.extend_from_slice(&(self.#f_ident.len() as u32).to_le_bytes());
                    // `Vec<T>` where `T: Pod` is already one contiguous,
                    // natively-encoded byte buffer — a single bulk cast+copy
                    // here instead of a per-element `bytemuck::bytes_of`
                    // loop, which cost one extra call per element for every
                    // T (not just Vec<u8>).
                    __buf.extend_from_slice(naclac_lang::prelude::bytemuck::cast_slice::<
                        #inner_ty,
                        u8,
                    >(&self.#f_ident));
                };
            }
            FieldKind::Option(inner_ty) => {
                size_expr = quote! {
                    #size_expr + 1 + if self.#f_ident.is_some() { core::mem::size_of::<#inner_ty>() } else { 0 }
                };
                write_stmts = quote! {
                    #write_stmts
                    match &self.#f_ident {
                        Some(__v) => {
                            __buf.push(1u8);
                            __buf.extend_from_slice(naclac_lang::prelude::bytemuck::bytes_of(__v));
                        }
                        None => {
                            __buf.push(0u8);
                        }
                    }
                };
            }
            FieldKind::Fixed => {
                size_expr = quote! { #size_expr + core::mem::size_of_val(&self.#f_ident) };
                write_stmts = quote! {
                    #write_stmts
                    __buf.extend_from_slice(naclac_lang::prelude::bytemuck::bytes_of(&self.#f_ident));
                };
            }
        }
    }

    let expanded = quote! {
        #[derive(Clone, Default)]
        #[cfg_attr(feature = "debug-mode", derive(Debug))]
        #ast

        impl #struct_name {
            /// The struct's raw field bytes, in the same sequential
            /// (length-prefixed String/Vec, tagged Option) encoding `emit`
            /// logs — without the 8-byte event discriminator. Useful for
            /// callers that need these exact bytes for something other than
            /// a log line (e.g. `set_return_data`).
            pub fn encode(&self) -> naclac_lang::prelude::Vec<u8> {
                let __total: usize = #size_expr;
                let mut __buf = naclac_lang::prelude::Vec::<u8>::with_capacity(__total);
                #write_stmts
                __buf
            }

            pub fn emit(&self) {
                let discriminator = [#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7];
                let __buf = self.encode();
                naclac_lang::prelude::sol_log_data(&[&discriminator, &__buf]);
            }
        }

        // Not `bytemuck::Pod` (String/Vec/Option fields), so it needs its own
        // explicit impl rather than relying on the blanket `T: Pod` one.
        impl naclac_lang::prelude::NaclacReturnData for #struct_name {
            fn to_return_data(&self) -> naclac_lang::prelude::Vec<u8> {
                self.encode()
            }
        }
    };

    TokenStream::from(expanded)
}
