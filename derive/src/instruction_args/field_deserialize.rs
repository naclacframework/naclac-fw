//! The statement that reads one `#[instruction_args]` field out of the
//! instruction data buffer in the zero-copy representation.

use crate::type_classify;

/// Generates a statement binding `field_name` by reading it off `data`
/// starting at `*offset`, advancing `*offset` past the consumed bytes.
///
/// The wire format matches `program.rs`'s per-argument dispatch: a fixed field
/// is read via `NaclacPod` at its known size; a dynamic field
/// (`ZcString`/`ZcVec<T>`/`Vec<T>`/`String`) is prefixed with a little-endian
/// `u32` *byte* length, matching `naclac-client-gen`'s
/// `generate_zero_copy_arg_bytes` on the write side. `Vec<Vec<u8>>` and
/// `Vec<String>` instead carry an element count, with each element
/// length-prefixed.
pub(super) fn field_deserialize_stmt(
    field_name: &syn::Ident,
    field_ty: &syn::Type,
) -> proc_macro2::TokenStream {
    use type_classify::DynamicKind;
    match type_classify::classify_dynamic(field_ty) {
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
