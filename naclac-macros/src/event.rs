//! # Event Macro Logic
//!
//! Handles the expansion of the `#[event]` macro, generating efficient event logging logic.
//! It computes the 8-byte discriminator at compile time and emits events using either
//! highly optimized zero-copy slicing (`bytemuck`), a single-allocation dynamic buffer
//! (`#[event(alloc)]`, for events needing `Vec`/`String`/`Option`), or standard Borsh
//! serialization.

use proc_macro::TokenStream;
use quote::quote;
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

/// Builds `impl #struct_name { pub fn __naclac_idl_event(...) -> IdlEvent }`
/// — mirrors real Anchor's own `__anchor_private_gen_idl_event` exactly:
/// events get a dedicated method (not the general `NaclacIdlBuild` trait,
/// since an `IdlEvent` is never itself referenced *by* another type's
/// `create_type()`), which both returns this event's shape and recursively
/// chases `NaclacIdlBuild::create_type()` for every field that references a
/// real defined type, inserting each into the shared `types` map the caller
/// passes in. Shared by both the plain and `#[event(alloc)]` branches, since
/// the IDL-visible shape doesn't depend on which wire encoding is active.
#[cfg(feature = "idl-build")]
fn idl_build_event_impl(
    struct_name: &syn::Ident,
    fields: &syn::Fields,
    disc: [u8; 8],
) -> proc_macro2::TokenStream {
    let mut field_ts_list = Vec::new();
    let mut defined_refs: Vec<Type> = Vec::new();
    for f in fields.iter() {
        let fname = f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default();
        let docs = naclac_syn::parser::extract_docs(&f.attrs);
        let ty_value = naclac_syn::parser::rust_type_to_idl(&f.ty, &[]);
        let Ok(ty_json) = serde_json::to_string(&ty_value) else {
            continue;
        };
        field_ts_list.push(quote! {
            naclac_lang::naclac_idl::IdlEventField {
                name: #fname.into(),
                docs: vec![#(#docs.into()),*],
                ty: naclac_lang::naclac_idl::serde_json::from_str(#ty_json)
                    .expect("naclac idl-build: generated field type JSON must parse"),
                index: false,
            }
        });
        if let Some(defined_ty) = naclac_syn::parser::defined_type_leaf(&f.ty) {
            defined_refs.push(defined_ty);
        }
    }

    let struct_name_str = struct_name.to_string();
    let [b0, b1, b2, b3, b4, b5, b6, b7] = disc;

    quote! {
        #[cfg(feature = "idl-build")]
        impl #struct_name {
            pub fn __naclac_idl_event(
                types: &mut naclac_lang::naclac_idl::__private::BTreeMap<
                    naclac_lang::naclac_idl::__private::String,
                    naclac_lang::naclac_idl::IdlTypeDef,
                >,
            ) -> naclac_lang::naclac_idl::IdlEvent {
                #(
                    if let Some(ty) = <#defined_refs as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::create_type() {
                        types.insert(
                            <#defined_refs as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::get_full_path(),
                            ty,
                        );
                        <#defined_refs as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::insert_types(types);
                    }
                )*
                naclac_lang::naclac_idl::IdlEvent {
                    name: #struct_name_str.into(),
                    docs: vec![],
                    discriminator: [#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7],
                    fields: vec![#(#field_ts_list),*],
                }
            }
        }
    }
}

#[cfg(not(feature = "idl-build"))]
fn idl_build_event_impl(
    _struct_name: &syn::Ident,
    _fields: &syn::Fields,
    _disc: [u8; 8],
) -> proc_macro2::TokenStream {
    quote! {}
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

    let result = naclac_syn::discriminator::compute_discriminator("event", &struct_name.to_string());

    let b0 = result[0];
    let b1 = result[1];
    let b2 = result[2];
    let b3 = result[3];
    let b4 = result[4];
    let b5 = result[5];
    let b6 = result[6];
    let b7 = result[7];

    let idl_build_impl = idl_build_event_impl(struct_name, &ast.fields, [b0, b1, b2, b3, b4, b5, b6, b7]);

    // Cargo gives a proc-macro no reliable way to see the invoking crate's
    // activated features (see `component.rs`'s doc comment for the full
    // story), so both representations are always emitted below, each gated
    // by a real `#[cfg(...)]` in the output that the calling crate's own
    // compiler resolves — same as `#[component]`/`#[derive(Accounts)]`. This
    // requires the calling crate to declare `pinocchio`/`borsh` as features
    // of its own (mirroring `naclac-lang`'s), which is now the enforced
    // convention for every naclac program crate.
    let zero_copy_cfg = quote! { #[cfg(any(feature = "pinocchio", not(feature = "borsh")))] };
    let borsh_cfg = quote! { #[cfg(all(not(feature = "pinocchio"), feature = "borsh"))] };

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
            &zero_copy_cfg,
            &borsh_cfg,
            idl_build_impl,
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

    let expanded = quote! {
        #zero_copy_cfg
        #[allow(non_upper_case_globals)]
        const #padding_const_name: usize = {
            let __total: usize = #size_sum;
            let __align: usize = #align_max;
            let __rem = __total % __align;
            if __rem == 0 { 0 } else { __align - __rem }
        };

        #zero_copy_cfg
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
        #zero_copy_cfg
        impl Default for #struct_name {
            fn default() -> Self {
                naclac_lang::prelude::bytemuck::Zeroable::zeroed()
            }
        }

        #zero_copy_cfg
        impl #struct_name {
            pub fn emit(&self) {
                let discriminator = [#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7];
                let data = naclac_lang::prelude::bytemuck::bytes_of(self);
                naclac_lang::prelude::sol_log_data(&[&discriminator, data]);
            }
        }

        #borsh_cfg
        #[derive(Clone, naclac_lang::prelude::BorshSerialize, Default)]
        #[cfg_attr(feature = "debug-mode", derive(Debug))]
        #[borsh(crate = "naclac_lang::prelude::borsh")]
        #ast

        #borsh_cfg
        impl #struct_name {
            pub fn emit(&self) {
                naclac_lang::event::emit_event(self, [#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7]);
            }
        }

        #idl_build_impl
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
    zero_copy_cfg: &proc_macro2::TokenStream,
    borsh_cfg: &proc_macro2::TokenStream,
    idl_build_impl: proc_macro2::TokenStream,
) -> TokenStream {
    // "Only for zero-copy backends" can no longer be checked at
    // macro-expansion time (see `expand`'s doc comment) — emitted as a
    // `compile_error!` behind the complementary `borsh_cfg` instead, so it
    // still only fires when the calling crate actually selects Borsh mode.
    let borsh_error = quote! {
        #borsh_cfg
        compile_error!(
            "Naclac Error: #[event(alloc)] is only for zero-copy backends (Pinocchio, or \
             Solana Program + Zero-Copy). Under Borsh mode, plain #[event] already supports \
             Vec/String/Option natively via real Borsh, so #[event(alloc)] is unnecessary \
             there — use plain #[event] instead."
        );
    };

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
        #borsh_error

        #zero_copy_cfg
        #[derive(Clone, Default)]
        #[cfg_attr(feature = "debug-mode", derive(Debug))]
        #ast

        #zero_copy_cfg
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
        #zero_copy_cfg
        impl naclac_lang::prelude::NaclacReturnData for #struct_name {
            fn to_return_data(&self) -> naclac_lang::prelude::Vec<u8> {
                self.encode()
            }
        }

        #idl_build_impl
    };

    TokenStream::from(expanded)
}
