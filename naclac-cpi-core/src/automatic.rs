//! # Automatic Instruction Extraction
//!
//! Provides utilities to traverse a module's AST and extract instruction metadata,
//! including Anchor-compatible 8-byte discriminators calculated via Sha256 hashes.

use crate::codegen::CpiInstruction;
use sha2::{Digest, Sha256};
use syn::{FnArg, Item, Pat, Path, Type};

/// Extracts CPI metadata from a module's items (for #[naclac_program])
pub fn extract_automatic_instructions(items: &[Item]) -> Vec<CpiInstruction> {
    let mut instructions = Vec::new();

    for item in items {
        if let Item::Fn(func) = item {
            // Only public functions are instructions
            if matches!(func.vis, syn::Visibility::Public(_)) {
                let func_name = func.sig.ident.clone();
                let mut arg_names_types = Vec::new();
                let mut context_struct = None;

                for arg in &func.sig.inputs {
                    if let FnArg::Typed(pat_type) = arg {
                        if let Pat::Ident(pat_ident) = &*pat_type.pat {
                            if let Some(ctx_path) = extract_context_path(&pat_type.ty) {
                                // Found the Context<Accounts> argument
                                context_struct = Some(ctx_path);
                            } else {
                                // Regular instruction argument
                                arg_names_types
                                    .push((pat_ident.ident.clone(), (*pat_type.ty).clone()));
                            }
                        }
                    }
                }

                let mut is_zero_copy = false;
                for attr in &func.attrs {
                    if attr.path().is_ident("instruction") {
                        let attr_str = quote::quote!(#attr).to_string();
                        if attr_str.contains("zero_copy") {
                            is_zero_copy = true;
                        }
                    }
                }

                if let Some(accounts_struct_path) = context_struct {
                    // Calculate Anchor-style 8-byte discriminator
                    let preimage = format!("global:{}", func_name);
                    let mut hasher = Sha256::new();
                    hasher.update(preimage.as_bytes());
                    let mut disc = [0u8; 8];
                    disc.copy_from_slice(&hasher.finalize()[..8]);

                    instructions.push(CpiInstruction {
                        name: func_name,
                        accounts_struct: accounts_struct_path,
                        args: arg_names_types,
                        discriminator: disc,
                        is_zero_copy,
                    });
                }
            }
        }
    }

    instructions
}

fn extract_context_path(ty: &Type) -> Option<Path> {
    let ty_path = match ty {
        Type::Path(p) => Some(p),
        Type::Reference(r) => {
            if let Type::Path(p) = &*r.elem {
                Some(p)
            } else {
                None
            }
        }
        _ => None,
    };

    if let Some(type_path) = ty_path {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Context" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    for arg in &args.args {
                        if let syn::GenericArgument::Type(Type::Path(inner_path)) = arg {
                            return Some(inner_path.path.clone());
                        }
                    }
                }
            }
        }
    }
    None
}
