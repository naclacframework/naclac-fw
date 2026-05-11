use crate::types::*;
use heck::ToLowerCamelCase;
use syn::Item;

use quote::ToTokens;
use serde_json::{Value, json};

fn get_type_size(ty: &syn::Type) -> usize {
    match ty {
        syn::Type::Path(type_path) => {
            let last_segment = type_path.path.segments.last().unwrap();
            let ident_str = last_segment.ident.to_string();
            match ident_str.as_str() {
                "u8" | "i8" | "bool" | "Bool" => 1,
                "u16" | "i16" => 2,
                "u32" | "i32" | "f32" => 4,
                "u64" | "i64" | "f64" => 8,
                "u128" | "i128" => 16,
                "Pubkey" | "publicKey" => 32,
                "Opt" => 48, // Opt<T> always 48 for simplicity (32+1+7 or 8+1+7 padded to 8)
                _ => 8,      // Default fallback
            }
        }
        syn::Type::Array(type_array) => {
            let inner_size = get_type_size(&type_array.elem);
            let len_str = type_array.len.to_token_stream().to_string().replace(" ", "");
            if let Ok(len_num) = len_str.parse::<usize>() {
                return inner_size * len_num;
            }
            0
        }
        _ => 0,
    }
}

pub fn rust_type_to_idl(ty: &syn::Type) -> Value {
    match ty {
        syn::Type::Path(type_path) => {
            let last_segment = type_path.path.segments.last().unwrap();
            let ident_str = last_segment.ident.to_string();
            
            if ident_str == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return json!({ "option": rust_type_to_idl(inner_ty) });
                    }
                }
            }
            
            if ident_str == "Vec" {
                if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return json!({ "vec": rust_type_to_idl(inner_ty) });
                    }
                }
            }

            match ident_str.as_str() {
                "Pubkey" => json!("publicKey"),
                "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32" | "i64" | "i128" | "f32" | "f64" => json!(ident_str),
                "bool" => json!("bool"),
                "Bool" => json!({ "defined": "Bool" }),
                "Opt" => {
                    if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                            let inner_ident = inner_ty.to_token_stream().to_string().replace(" ", "");
                            let normalized_ident = match inner_ident.as_str() {
                                "u64" => "U64",
                                "Pubkey" | "publicKey" => "Pubkey",
                                _ => &inner_ident,
                            };
                            return json!({ "defined": format!("Opt{}", normalized_ident) });
                        }
                    }
                    json!({ "defined": "Opt" })
                },
                "String" => json!("string"),
                _ => json!({ "defined": ident_str }), 
            }
        }
        syn::Type::Slice(type_slice) => {
            let inner = rust_type_to_idl(&type_slice.elem);
            if inner == json!("u8") {
                return json!("bytes")
            }
            json!({ "vec": inner })
        }
        syn::Type::Array(type_array) => {
            let inner_ty = rust_type_to_idl(&type_array.elem);
            let len_str = type_array.len.to_token_stream().to_string().replace(" ", "");
            if let Ok(len_num) = len_str.parse::<usize>() {
                return json!({ "array": [inner_ty, len_num] });
            }
            json!("bytes")
        }
        syn::Type::Reference(type_ref) => {
            if let syn::Type::Path(type_path) = &*type_ref.elem {
                if type_path.path.segments.last().unwrap().ident == "str" {
                    return json!("string");
                }
            }
            rust_type_to_idl(&type_ref.elem)
        }
        _ => {
            let raw = ty.to_token_stream().to_string().replace(" ", "");
            json!(raw)
        }
    }
}

pub fn parse_file(idl: &mut NaclacProgram, code: &str) {
    if let Ok(syntax_tree) = syn::parse_file(code) {
        for item in &syntax_tree.items {
            match item {
                Item::Struct(item_struct) => {
                    // Check #[component]
                    let is_component = item_struct.attrs.iter().any(|a| a.path().is_ident("component"));
                    if is_component {
                        let mut fields = Vec::new();
                        for field in &item_struct.fields {
                            let field_name = field.ident.as_ref().unwrap().to_string().trim_start_matches('_').to_string().to_lower_camel_case();
                            fields.push(NaclacField {
                                name: field_name,
                                ty: rust_type_to_idl(&field.ty),
                            });
                        }
                        idl.accounts.push(NaclacAccountStruct {
                            name: item_struct.ident.to_string(),
                            fields,
                        });
                        continue;
                    }

                    // Check #[event]
                    let is_event = item_struct.attrs.iter().any(|a| a.path().is_ident("event"));
                    if is_event {
                        let mut fields = Vec::new();
                        let mut total_size = 0;
                        for field in &item_struct.fields {
                            let field_name = field.ident.as_ref().unwrap().to_string().trim_start_matches('_').to_string().to_lower_camel_case();
                            let ty_val = rust_type_to_idl(&field.ty);
                            let field_size = get_type_size(&field.ty);
                            total_size += field_size;

                            fields.push(NaclacEventField {
                                name: field_name,
                                ty: ty_val,
                                index: false,
                            });
                        }

                        // Apply Zero-Copy Padding if needed
                        if idl.is_zero_copy && total_size % 8 != 0 {
                            let padding_size = 8 - (total_size % 8);
                            fields.push(NaclacEventField {
                                name: "padding".to_string(),
                                ty: json!({ "array": ["u8", padding_size] }),
                                index: false,
                            });
                        }

                        idl.events.push(NaclacEvent {
                            name: item_struct.ident.to_string(),
                            fields,
                        });
                        continue;
                    }
                }
                Item::Enum(item_enum) => {
                    let is_error = item_enum.attrs.iter().any(|a| a.path().is_ident("error_code"));
                    if is_error {
                        let mut code_offset = 6000;
                        for variant in &item_enum.variants {
                            let name = variant.ident.to_string();
                            let mut msg = None;
                            for attr in &variant.attrs {
                                if attr.path().is_ident("doc") {
                                    if let syn::Meta::NameValue(mnv) = &attr.meta {
                                        if let syn::Expr::Lit(expr_lit) = &mnv.value {
                                            if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                                                let doc_str = lit_str.value().trim().to_string();
                                                msg = Some(if let Some(existing) = msg {
                                                    format!("{} {}", existing, doc_str)
                                                } else {
                                                    doc_str
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                            idl.errors.push(NaclacError {
                                code: code_offset,
                                name,
                                msg,
                            });
                            code_offset += 1;
                        }
                    } else {
                        // Generic Data Enum
                        let mut variants = Vec::new();
                        for variant in &item_enum.variants {
                            variants.push(NaclacEnumVariant {
                                name: variant.ident.to_string(),
                            });
                        }
                        idl.types.push(NaclacTypeDef {
                            name: item_enum.ident.to_string(),
                            ty: NaclacTypeDefTy::Enum { variants },
                        });
                    }
                }
                Item::Const(item_const) => {
                    let is_pub = matches!(item_const.vis, syn::Visibility::Public(_));
                    let is_exported = item_const.attrs.iter().any(|a| a.path().is_ident("constant"));
                    if is_pub || is_exported {
                        let expr_str = item_const.expr.to_token_stream().to_string().replace(" ", "");
                        idl.constants.push(NaclacConstant {
                            name: item_const.ident.to_string(),
                            ty: rust_type_to_idl(&item_const.ty),
                            value: expr_str,
                            is_exported,
                        });
                    }
                }
                _ => {}
            }
        }
    }
}
