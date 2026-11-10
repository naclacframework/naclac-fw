//! Deserializing a handler's instruction arguments out of `instruction_data`.
//! Every layout is generated, each behind its own `#[cfg(feature = ...)]`
//! resolved by the calling crate's compiler.

use {
    crate::{
        common::cfg_gates,
        type_classify::{self, DynamicKind},
    },
    proc_macro2::TokenStream,
    quote::quote,
    std::collections::HashSet,
    syn::{Ident, Type},
};

/// The generated argument-reading code of one handler.
pub(super) struct ArgsCode<'a> {
    /// Binds every non-slice argument from `instruction_data`, starting at the
    /// discriminator length.
    pub(super) deserialization: TokenStream,
    /// Arguments typed `&[u8]`, which borrow the rest of `instruction_data`
    /// instead of being deserialized.
    pub(super) slice_arg_names: Vec<&'a Ident>,
}

/// How an argument's declared type is classified.
struct ArgKind {
    zero_copy_vec: bool,
    zero_copy_string: bool,
    heap_vec: bool,
    heap_string: bool,
}

impl ArgKind {
    fn of(ty: &Type) -> Self {
        let kind = type_classify::classify_dynamic(ty);
        ArgKind {
            zero_copy_vec: matches!(kind, DynamicKind::ZcVec),
            zero_copy_string: matches!(kind, DynamicKind::ZcString),
            heap_vec: matches!(kind, DynamicKind::HeapVec),
            heap_string: matches!(kind, DynamicKind::HeapString),
        }
    }

    fn is_zero_copy_collection(&self) -> bool {
        self.zero_copy_vec || self.zero_copy_string
    }

    fn is_heap_collection(&self) -> bool {
        self.heap_vec || self.heap_string
    }
}

/// The argument-reading code for the handler arguments `arg_names`/`arg_types`.
pub(super) fn deserialize<'a>(
    arg_names: &[&'a Ident],
    arg_types: &[&Type],
    heap_allowed_args: &HashSet<Ident>,
    ix_disc_len: usize,
) -> ArgsCode<'a> {
    let zero_copy_cfg = cfg_gates::zero_copy();
    let borsh_cfg = cfg_gates::borsh();
    let wincode_cfg = cfg_gates::wincode();

    let mut deserialization_logic = Vec::new();
    let mut slice_arg_names = Vec::new();
    // `__ix_offset` is declared only when some argument needs it, so an
    // instruction with no arguments gets no dead code.
    let mut has_non_slice_args = false;

    for (name, ty) in arg_names.iter().zip(arg_types.iter()) {
        if type_classify::is_byte_slice_ref(ty) {
            slice_arg_names.push(*name);
            continue;
        }
        has_non_slice_args = true;

        let kind = ArgKind::of(ty);
        let zero_copy_stmt = zero_copy_stmt(name, ty, &kind);
        let borsh_stmt = borsh_stmt(name, ty, &kind);
        let wincode_stmt = wincode_stmt(name, ty, &kind);

        // Only the zero-copy layout has a `ZcVec`/`ZcString` to nudge toward.
        let mut arg_stmt = quote! {};
        if kind.is_heap_collection() && !heap_allowed_args.contains(*name) {
            let found_ty = if kind.heap_vec { "Vec<T>" } else { "String" };
            let suggested_ty = if kind.heap_vec {
                "ZcVec<T>"
            } else {
                "ZcString"
            };
            let warning = crate::common::diagnostics::heap_collection_warning(
                &name.to_string(),
                found_ty,
                suggested_ty,
            );
            arg_stmt = quote! { #arg_stmt #zero_copy_cfg { #warning } };
        }

        deserialization_logic.push(quote! {
            #arg_stmt
            #zero_copy_cfg
            #zero_copy_stmt
            #borsh_cfg
            #borsh_stmt
            #wincode_cfg
            #wincode_stmt
        });
    }

    let deserialization = if has_non_slice_args {
        quote! {
            let mut __ix_offset: usize = #ix_disc_len;
            #( #deserialization_logic )*
        }
    } else {
        quote! { #( #deserialization_logic )* }
    };
    ArgsCode {
        deserialization,
        slice_arg_names,
    }
}

/// The zero-copy statement reading one argument. `NaclacArgs` (not
/// `NaclacPod`) covers both a fixed-size argument and an `#[instruction_args]`
/// struct holding a dynamic field, whose encoded length depends on its content.
fn zero_copy_stmt(name: &Ident, ty: &Type, kind: &ArgKind) -> TokenStream {
    let is_zero_copy_vec = kind.zero_copy_vec;
    let is_zero_copy_collection = kind.is_zero_copy_collection();
    let is_heap_vec = kind.heap_vec;
    let is_heap_string = kind.heap_string;
    let is_heap_collection = kind.is_heap_collection();
    if !is_zero_copy_collection && !is_heap_collection {
        // `NaclacArgs` (not `NaclacPod` directly) — covers both a
        // plain fixed-size arg (via `NaclacPod`'s blanket
        // `NaclacArgs` impl) and an `#[instruction_args]`-grouped
        // struct containing a `ZcString`/`ZcVec` field, whose
        // encoded length depends on its own content and can
        // never be read via a single raw `size_of`-based cast.
        quote! {
            let #name = <#ty as naclac_lang::prelude::NaclacArgs>::naclac_deserialize(
                instruction_data, &mut __ix_offset
            )?;
        }
    } else {
        let inner_ty = type_classify::first_generic_type(ty);

        let __v_logic = if is_zero_copy_collection {
            let bounded_end = quote! {
                let __end = match __ix_offset.checked_add(__len) {
                    Some(end) if end <= instruction_data.len() => end,
                    _ => return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)),
                };
            };
            if is_zero_copy_vec && inner_ty.is_none() {
                return quote! {
                    compile_error!("Naclac Error: `ZcVec` needs an element type, e.g. `ZcVec<u8>`.");
                };
            }
            let view_ty = type_classify::zc_view_type(ty);
            quote! {
                #bounded_end
                let __v = <#view_ty>::from_bytes_bounded(&instruction_data[__ix_offset..__end])?;
                __ix_offset = __end;
                __v
            }
        } else {
            let is_nested_vec = is_heap_vec
                && inner_ty.is_some_and(|inner| {
                    type_classify::is_exactly(inner, "Vec")
                        || type_classify::is_exactly(inner, "String")
                });
            if is_heap_string {
                quote! {
                    let __bytes_len = __len as usize;
                    if instruction_data.len() < __ix_offset + __bytes_len {
                        return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                    }
                    let __v = naclac_lang::prelude::String::from_utf8(instruction_data[__ix_offset..__ix_offset+__bytes_len].to_vec())
                        .map_err(|_| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?;
                    __ix_offset += __bytes_len;
                    __v
                }
            } else if is_nested_vec {
                quote! {
                    naclac_lang::prelude::read_length_prefixed_items(
                        instruction_data,
                        &mut __ix_offset,
                        __len,
                    )?
                }
            } else {
                if let Some(inner) = &inner_ty {
                    if type_classify::is_exactly(inner, "u8") {
                        quote! {
                            let __bytes_len = __len as usize;
                            if instruction_data.len() < __ix_offset + __bytes_len {
                                return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                            }
                            let mut __v = naclac_lang::prelude::Vec::with_capacity(__bytes_len);
                            __v.extend_from_slice(&instruction_data[__ix_offset..__ix_offset+__bytes_len]);
                            __ix_offset += __bytes_len;
                            __v
                        }
                    } else {
                        quote! {
                            let __sz = <#inner as naclac_lang::prelude::NaclacPod>::naclac_size();
                            if __sz == 0
                                || __len % __sz != 0
                                || instruction_data.len() < __ix_offset + __len
                            {
                                return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                            }
                            let mut __v = naclac_lang::prelude::Vec::with_capacity(__len / __sz);
                            let __end = __ix_offset + __len;
                            while __ix_offset < __end {
                                __v.push(
                                    <#inner as naclac_lang::prelude::NaclacPod>::naclac_try_from_bytes(
                                        &instruction_data[__ix_offset..__ix_offset + __sz]
                                    )
                                    .ok_or_else(|| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?,
                                );
                                __ix_offset += __sz;
                            }
                            __v
                        }
                    }
                } else {
                    quote! {
                        compile_error!("Naclac Error: `Vec` needs an element type, e.g. `Vec<u8>`.");
                    }
                }
            }
        };

        quote! {
            let #name = {
                if instruction_data.len() < __ix_offset + 4 {
                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                }
                let mut __len_bytes = [0u8; 4];
                __len_bytes.copy_from_slice(&instruction_data[__ix_offset..__ix_offset+4]);
                let __len = u32::from_le_bytes(__len_bytes) as usize;
                __ix_offset += 4;
                #__v_logic
            };
        }
    }
}

/// The Borsh statement reading one argument. Borsh decodes `Vec<T>`/`String`
/// natively, so one `BorshDeserialize` call covers every other shape.
/// `ZcVec<T>`/`ZcString` are views with no bytes of their own, so a purpose-built
/// error replaces the call. That error still binds `name` (to a `!`-typed
/// `unreachable!()`), as the handler call references it later and an unbound
/// name would add a second, unrelated error.
fn borsh_stmt(name: &Ident, ty: &Type, kind: &ArgKind) -> TokenStream {
    let is_zero_copy_vec = kind.zero_copy_vec;
    let is_zero_copy_collection = kind.is_zero_copy_collection();
    if is_zero_copy_collection {
        let found_ty = if is_zero_copy_vec {
            "ZcVec<T>"
        } else {
            "ZcString"
        };
        let msg = format!(
            "Naclac Error: argument '{}' has type '{}', which Borsh can't encode (it is a view \
             into the instruction data). Use Vec<T> or String.",
            name, found_ty
        );
        quote! {
            let #name: #ty = { compile_error!(#msg); (|| -> #ty { unreachable!() })() };
        }
    } else {
        quote! {
            let #name = {
                let mut __r: &[u8] = &instruction_data[__ix_offset..];
                let __val = <#ty as naclac_lang::prelude::BorshDeserialize>::deserialize(&mut __r)
                    .map_err(|_| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?;
                __ix_offset = instruction_data.len() - __r.len();
                __val
            };
        }
    }
}

/// The Wincode statement reading one argument; see `borsh_stmt` for why a
/// `ZcVec`/`ZcString` becomes an error.
fn wincode_stmt(name: &Ident, ty: &Type, kind: &ArgKind) -> TokenStream {
    let is_zero_copy_vec = kind.zero_copy_vec;
    let is_zero_copy_collection = kind.is_zero_copy_collection();
    if is_zero_copy_collection {
        let found_ty = if is_zero_copy_vec {
            "ZcVec<T>"
        } else {
            "ZcString"
        };
        let msg = format!(
            "Naclac Error: argument '{}' has type '{}', which Wincode can't encode (it is a \
             view into the instruction data). Use Vec<T> or String.",
            name, found_ty
        );
        quote! {
            let #name: #ty = { compile_error!(#msg); (|| -> #ty { unreachable!() })() };
        }
    } else {
        quote! {
            let #name = {
                let mut __r: &[u8] = &instruction_data[__ix_offset..];
                let __val: #ty = naclac_lang::prelude::wincode::config::deserialize_from(&mut __r, naclac_lang::prelude::wincode_config::CONFIG)
                    .map_err(|_| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?;
                __ix_offset = instruction_data.len() - __r.len();
                __val
            };
        }
    }
}
