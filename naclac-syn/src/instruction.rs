use crate::types::*;
use crate::parser::rust_type_to_idl;
use crate::pda::parse_pda_seeds;
use heck::ToLowerCamelCase;
use quote::quote;
use syn::Item;
use std::collections::HashMap;

pub fn extract_instructions(idl: &mut NaclacProgram, codes: &[String], func_order: &[String]) {
    let mut account_structs_map: HashMap<String, Vec<NaclacAccount>> = HashMap::new();

    // PASS 1: Build a map of struct_name -> Vec<NaclacAccount>
    for code in codes {
        if let Ok(syntax_tree) = syn::parse_file(code) {
            for item in &syntax_tree.items {
                if let Item::Struct(item_struct) = item {
                    let mut account_names = Vec::new();
                    for field in &item_struct.fields {
                        if let Some(ident) = &field.ident {
                            account_names.push(ident.to_string());
                        }
                    }

                    let mut accounts = Vec::new();
                    for field in &item_struct.fields {
                        let field_name = field.ident.as_ref().unwrap().to_string();
                        let mut is_mut = false;
                        let mut is_signer = false;
                        let mut meta_list = None;

                        let mut is_program = false;
                        let mut is_sysvar = false;
                        let mut inner_type = String::new();
                        if let syn::Type::Path(type_path) = &field.ty {
                            if let Some(last_segment) = type_path.path.segments.last() {
                                if last_segment.ident == "Signer" {
                                    is_signer = true;
                                } else if last_segment.ident == "Program" {
                                    is_program = true;
                                    if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                                        for arg in &args.args {
                                            if let syn::GenericArgument::Type(syn::Type::Path(p)) = arg {
                                                if let Some(inner_seg) = p.path.segments.last() {
                                                    inner_type = inner_seg.ident.to_string();
                                                }
                                            }
                                        }
                                    }
                                } else if last_segment.ident == "Sysvar" || last_segment.ident == "Rent" {
                                    is_sysvar = true;
                                    if last_segment.ident == "Rent" {
                                        inner_type = "Rent".to_string();
                                    }
                                }
                            }
                        }

                        for attr in &field.attrs {
                            let attr_str = quote!(#attr).to_string();
                            if attr_str.contains("mut") || attr_str.contains("init") {
                                is_mut = true;
                            }
                            if let syn::Meta::List(list) = &attr.meta {
                                if list.path.is_ident("account") {
                                    meta_list = Some(list.clone());
                                }
                            }
                        }

                        let pda = if let Some(meta) = meta_list.clone() {
                            parse_pda_seeds(&meta, &account_names, &idl.constants)
                        } else {
                            None
                        };
                        
                        let mut address = None;
                        if let Some(ref meta) = meta_list {
                            if let Ok(punctuated) = meta.parse_args_with(syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated) {
                                for m in punctuated {
                                    if let syn::Meta::NameValue(mnv) = m {
                                        if mnv.path.is_ident("address") {
                                            if let syn::Expr::Path(expr_path) = mnv.value {
                                                // Only resolve if this is a local single-ident constant (not an external crate path)
                                                let is_external = expr_path.path.segments.len() > 1;
                                                let ident = expr_path.path.segments.last().unwrap().ident.to_string();
                                                
                                                if let Some(c) = idl.constants.iter().find(|c| c.name == ident) {
                                                    let val = c.value.clone();
                                                    if val.starts_with("\"") && val.ends_with("\"") {
                                                        address = Some(val[1..val.len() - 1].to_string());
                                                    } else {
                                                        address = Some(val);
                                                    }
                                                } else {
                                                    let path_str = quote::quote!(#expr_path).to_string().replace(" ", "");
                                                    if path_str.contains("System") {
                                                        address = Some("11111111111111111111111111111111".to_string());
                                                    } else if path_str.contains("Token2022") || path_str.contains("token_2022") {
                                                        address = Some("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".to_string());
                                                    } else if path_str.contains("Token") || path_str.contains("token") {
                                                        address = Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string());
                                                    } else if path_str.contains("Rent") {
                                                        address = Some("SysvarRent111111111111111111111111111111111".to_string());
                                                    } else if !is_external {
                                                        // Local single-ident but unresolved — store ident as placeholder
                                                        address = Some(ident);
                                                    }
                                                }
                                                // If external crate path and unresolved: leave address as None
                                            } else if let syn::Expr::Lit(expr_lit) = mnv.value {
                                                if let syn::Lit::Str(lit_str) = expr_lit.lit {
                                                    address = Some(lit_str.value());
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                // FALLBACK PASS: Manually parse TokenStream
                                let tokens: Vec<_> = meta.tokens.clone().into_iter().collect();
                                for (i, token) in tokens.iter().enumerate() {
                                    if let proc_macro2::TokenTree::Ident(ident) = token {
                                        if ident == "address" && i + 2 < tokens.len() {
                                            if let proc_macro2::TokenTree::Punct(punct) = &tokens[i + 1] {
                                                if punct.as_char() == '=' {
                                                    let mut addr_tokens = proc_macro2::TokenStream::new();
                                                    for tgt in &tokens[i + 2..] {
                                                        if let proc_macro2::TokenTree::Punct(p) = tgt {
                                                            if p.as_char() == ',' { break; }
                                                        }
                                                        addr_tokens.extend(std::iter::once(tgt.clone()));
                                                    }
                                                    if let Ok(expr) = syn::parse2::<syn::Expr>(addr_tokens) {
                                                        if let syn::Expr::Path(expr_path) = expr {
                                                            let is_external = expr_path.path.segments.len() > 1;
                                                            let c_ident = expr_path.path.segments.last().unwrap().ident.to_string();
                                                            if let Some(c) = idl.constants.iter().find(|c| c.name == c_ident) {
                                                                let val = c.value.clone();
                                                                if val.starts_with("\"") && val.ends_with("\"") {
                                                                    address = Some(val[1..val.len() - 1].to_string());
                                                                } else {
                                                                    address = Some(val);
                                                                }
                                                            } else {
                                                                let path_str = quote::quote!(#expr_path).to_string().replace(" ", "");
                                                                if path_str.contains("System") {
                                                                    address = Some("11111111111111111111111111111111".to_string());
                                                                } else if path_str.contains("Token2022") || path_str.contains("token_2022") {
                                                                    address = Some("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".to_string());
                                                                } else if path_str.contains("Token") || path_str.contains("token") {
                                                                    address = Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string());
                                                                } else if path_str.contains("Rent") {
                                                                    address = Some("SysvarRent111111111111111111111111111111111".to_string());
                                                                } else if !is_external {
                                                                    address = Some(c_ident);
                                                                }
                                                            }
                                                        } else if let syn::Expr::Lit(expr_lit) = expr {
                                                            if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                                                                address = Some(lit_str.value());
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if address.is_none() {
                            let name_lower = field_name.to_lowercase();
                            if is_program {
                                if inner_type == "Token2022" {
                                    address = Some("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".to_string());
                                } else if inner_type == "Token" {
                                    address = Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string());
                                } else if inner_type == "System" || name_lower == "system_program" {
                                    address = Some("11111111111111111111111111111111".to_string());
                                } else if inner_type == "AssociatedToken" || name_lower == "associated_token_program" {
                                    address = Some("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL".to_string());
                                }
                            } else if is_sysvar {
                                if inner_type == "Rent" || name_lower == "rent" {
                                    address = Some("SysvarRent111111111111111111111111111111111".to_string());
                                }
                            }
                        }

                        accounts.push(NaclacAccount {
                            name: field_name.trim_start_matches('_').to_string().to_lower_camel_case(),
                            writable: is_mut,
                            signer: is_signer,
                            pda,
                            address,
                        });
                    }
                    account_structs_map.insert(item_struct.ident.to_string(), accounts);
                }
            }
        }
    }

    // PASS 2: Match func_order, extract args, and grab the accounts
    for func_name in func_order {
        for code in codes {
            if let Ok(syntax_tree) = syn::parse_file(code) {
                for item in &syntax_tree.items {
                    if let Item::Fn(item_fn) = item {
                        if item_fn.sig.ident.to_string() == *func_name {
                            let mut args = Vec::new();
                            let mut context_struct_name = Option::<String>::None;

                            for arg in &item_fn.sig.inputs {
                                if let syn::FnArg::Typed(pat_type) = arg {
                                    let raw_arg_name = if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                                        pat_ident.ident.to_string()
                                    } else {
                                        continue;
                                    };

                                    let mut is_context = false;
                                    if let syn::Type::Path(type_path) = &*pat_type.ty {
                                        if let Some(last_segment) = type_path.path.segments.last() {
                                            if last_segment.ident == "Context" {
                                                is_context = true;
                                                if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                                                    if let Some(syn::GenericArgument::Type(syn::Type::Path(inner_path))) = args.args.last() {
                                                        if let Some(inner_last) = inner_path.path.segments.last() {
                                                            context_struct_name = Some(inner_last.ident.to_string());
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    if is_context {
                                        continue;
                                    }

                                    let arg_name = raw_arg_name.trim_start_matches('_').to_string().to_lower_camel_case();
                                    args.push(NaclacField {
                                        name: arg_name,
                                        ty: rust_type_to_idl(&*pat_type.ty),
                                    });
                                }
                            }

                            let mut accounts = Vec::new();
                            if let Some(ref struct_name) = context_struct_name {
                                if let Some(found_accounts) = account_structs_map.get(struct_name) {
                                    accounts.extend(found_accounts.clone());
                                }
                            }

                            let mut disc = [0u8; 8];
                            use sha2::{Digest, Sha256};
                            let preimage = format!("global:{}", func_name);
                            let mut hasher = Sha256::new();
                            hasher.update(preimage.as_bytes());
                            disc.copy_from_slice(&hasher.finalize()[..8]);

                            let camel_name = func_name.to_lower_camel_case();
                            if !idl.instructions.iter().any(|ix| ix.name == camel_name) {
                                idl.instructions.push(NaclacInstruction {
                                    name: camel_name,
                                    discriminator: disc,
                                    accounts,
                                    args,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}
