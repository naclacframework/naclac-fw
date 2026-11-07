//! The `#[instruction(...)]` arguments of an Accounts struct: parsing them off
//! the struct and generating, per layout, the code that reads them out of the
//! instruction data inside `load_and_validate`.

use {
    super::layout::Layout,
    crate::{
        common::diagnostics::heap_collection_warning,
        type_classify::{self, DynamicKind},
    },
    proc_macro2::TokenStream,
    quote::quote,
    std::collections::HashSet,
};

pub(super) struct InstructionArgs {
    pub(super) names: Vec<syn::Ident>,
    pub(super) types: Vec<syn::Type>,
    /// Arguments marked `#[allow_heap]`, which skip the heap-collection
    /// warning. The marker needs no stripping here: it sits inside
    /// `#[instruction(...)]`, whose contents rustc never validates.
    heap_allowed: HashSet<syn::Ident>,
    /// A malformed `#[instruction(...)]`. It is embedded as a `compile_error!`
    /// in the deserializer and not returned early, so the struct's `Bumps`,
    /// `LoadableAccounts` and teardown impls still generate and the single
    /// real error is not buried under errors from the macros that need them.
    parse_error: Option<syn::Error>,
}

impl InstructionArgs {
    pub(super) fn parse(attrs: &[syn::Attribute]) -> Self {
        let mut args = InstructionArgs {
            names: Vec::new(),
            types: Vec::new(),
            heap_allowed: HashSet::new(),
            parse_error: None,
        };
        for attr in attrs {
            if !attr.path().is_ident("instruction") {
                continue;
            }
            match attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::FnArg, syn::Token![,]>::parse_terminated,
            ) {
                Ok(parsed_args) => {
                    for arg in parsed_args {
                        if let syn::FnArg::Typed(pat_type) = arg {
                            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                                if pat_type
                                    .attrs
                                    .iter()
                                    .any(|a| a.path().is_ident("allow_heap"))
                                {
                                    args.heap_allowed.insert(pat_ident.ident.clone());
                                }
                                args.names.push(pat_ident.ident.clone());
                                args.types.push((*pat_type.ty).clone());
                            }
                        }
                    }
                }
                Err(err) => args.parse_error = Some(err),
            }
        }
        args
    }

    /// The statements binding every instruction argument from
    /// `instruction_data`, for `layout`.
    pub(super) fn deserializer(&self, layout: Layout) -> TokenStream {
        if let Some(err) = &self.parse_error {
            return err.to_compile_error();
        }
        if self.names.is_empty() {
            return quote! {};
        }
        let names = &self.names;
        let types = &self.types;
        match layout {
            Layout::ZeroCopy => {
                let reads = names
                    .iter()
                    .zip(types.iter())
                    .map(|(name, ty)| self.zero_copy_read(name, ty));
                quote! {
                    let mut __ix_offset: usize = __NACLAC_IX_DISC_LEN;
                    #(#reads)*
                    #(let _ = &#names;)*
                }
            }
            Layout::Borsh => quote! {
                let mut __r: &[u8] = &instruction_data[__NACLAC_IX_DISC_LEN..];
                #(
                    let #names = <#types as naclac_lang::prelude::BorshDeserialize>::deserialize(&mut __r)
                        .map_err(|_| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?;
                )*
                #(let _ = &#names;)*
            },
            Layout::Wincode => quote! {
                let mut __r: &[u8] = &instruction_data[__NACLAC_IX_DISC_LEN..];
                #(
                    let #names : #types = naclac_lang::prelude::wincode::config::deserialize_from(&mut __r, naclac_lang::prelude::wincode_config::CONFIG)
                        .map_err(|_| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?;
                )*
                #(let _ = &#names;)*
            },
        }
    }

    /// The statement reading one argument in the zero-copy layout. Arguments
    /// are read in declaration order and must stay in step with the argument
    /// parser in `program.rs`: both read the same instruction data, and the
    /// same source type must mean the same thing in both.
    fn zero_copy_read(&self, name: &syn::Ident, ty: &syn::Type) -> TokenStream {
        if type_classify::is_byte_slice_ref(ty) {
            return quote! { let #name = &instruction_data[__ix_offset..]; };
        }

        let kind = type_classify::classify_dynamic(ty);

        if matches!(kind, DynamicKind::Fixed | DynamicKind::OptionPod) {
            // `NaclacArgs`, not `NaclacPod`: it covers a fixed-size argument
            // (through `NaclacPod`'s blanket impl), a bare `Option<T>` and an
            // `#[instruction_args]` struct holding a dynamic field alike.
            return quote! {
                let #name = <#ty as naclac_lang::prelude::NaclacArgs>::naclac_deserialize(
                    instruction_data, &mut __ix_offset
                )?;
            };
        }

        let is_zc_vec = matches!(kind, DynamicKind::ZcVec);
        let is_zc_string = matches!(kind, DynamicKind::ZcString);
        let is_heap_vec = matches!(kind, DynamicKind::HeapVec);
        let is_heap_string = matches!(kind, DynamicKind::HeapString);

        let inner_ty = type_classify::first_generic_type(ty);

        // `Vec<Vec<u8>>`/`Vec<String>`: the prefix is an element count and the
        // reader advances the offset itself, so it cannot share the byte-length
        // block below.
        let is_list_of_variable_items = is_heap_vec
            && inner_ty.is_some_and(|inner| {
                type_classify::is_exactly(inner, "Vec")
                    || type_classify::is_exactly(inner, "String")
            });
        if is_list_of_variable_items {
            let warning_tokens = if self.heap_allowed.contains(name) {
                quote! {}
            } else {
                heap_collection_warning(&name.to_string(), "Vec<T>", "ZcVec<T>")
            };
            return quote! {
                #warning_tokens
                let #name = {
                    if instruction_data.len() < __ix_offset + 4 {
                        return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                    }
                    let mut __count_bytes = [0u8; 4];
                    __count_bytes.copy_from_slice(&instruction_data[__ix_offset..__ix_offset+4]);
                    let __count = u32::from_le_bytes(__count_bytes) as usize;
                    __ix_offset += 4;
                    naclac_lang::prelude::read_length_prefixed_items(
                        instruction_data,
                        &mut __ix_offset,
                        __count,
                    )?
                };
            };
        }

        let value = if is_zc_vec {
            if let Some(inner) = inner_ty {
                quote! { <naclac_lang::prelude::ZcVec<#inner>>::from_bytes(&instruction_data[__ix_offset..__ix_offset + __len])? }
            } else {
                quote! { <naclac_lang::prelude::ZcVec<u8>>::from_bytes(&instruction_data[__ix_offset..__ix_offset + __len])? }
            }
        } else if is_zc_string {
            quote! { <naclac_lang::prelude::ZcString>::from_bytes(&instruction_data[__ix_offset..__ix_offset + __len])? }
        } else if is_heap_string {
            quote! {
                naclac_lang::prelude::String::from_utf8(instruction_data[__ix_offset..__ix_offset+__len].to_vec())
                    .map_err(|_| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?
            }
        } else if let Some(inner) = inner_ty {
            if type_classify::is_exactly(inner, "u8") {
                quote! { instruction_data[__ix_offset..__ix_offset+__len].to_vec() }
            } else {
                quote! {
                    {
                        let __sz = <#inner as naclac_lang::prelude::NaclacPod>::naclac_size();
                        if __sz == 0 || __len % __sz != 0 {
                            return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                        }
                        let mut __v = naclac_lang::prelude::Vec::with_capacity(__len / __sz);
                        let mut __p = __ix_offset;
                        while __p < __ix_offset + __len {
                            __v.push(
                                <#inner as naclac_lang::prelude::NaclacPod>::naclac_try_from_bytes(
                                    &instruction_data[__p..__p + __sz]
                                )
                                .ok_or_else(|| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?,
                            );
                            __p += __sz;
                        }
                        __v
                    }
                }
            }
        } else {
            quote! { return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)); }
        };

        let warning_tokens = if (is_heap_vec || is_heap_string) && !self.heap_allowed.contains(name)
        {
            let found_ty = if is_heap_vec { "Vec<T>" } else { "String" };
            let suggested_ty = if is_heap_vec { "ZcVec<T>" } else { "ZcString" };
            heap_collection_warning(&name.to_string(), found_ty, suggested_ty)
        } else {
            quote! {}
        };

        quote! {
            #warning_tokens
            let #name = {
                if instruction_data.len() < __ix_offset + 4 {
                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                }
                let mut __len_bytes = [0u8; 4];
                __len_bytes.copy_from_slice(&instruction_data[__ix_offset..__ix_offset+4]);
                let __len = u32::from_le_bytes(__len_bytes) as usize;
                __ix_offset += 4;
                if instruction_data.len() < __ix_offset + __len {
                    return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                }
                let __v = #value;
                __ix_offset += __len;
                __v
            };
        }
    }
}
