//! # Instruction Macro Module
//!
//! Houses the expansion logic for the `#[instruction]` macro. This macro rewrites
//! instruction function signatures to properly enforce Zero-Copy `Hydrated` accounts
//! and manages the generation of security checks, CPI initializers, and account reallocation.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

pub mod close_account;
pub mod init_cpi;
pub mod parser;
pub mod realloc;
pub mod security;

/// Expands the `#[instruction]` macro.
///
/// This macro rewrites the developer's instruction function signature, automatically converting
/// `Context<T>` to `Context<'info, THydrated<'info>>`. This ensures that all accounts within
/// the instruction body are safely type-guarded and correctly bound to the `'info` lifetime.
pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input_fn = parse_macro_input!(item as ItemFn);
    let fn_vis = &input_fn.vis;
    let fn_block = &input_fn.block;
    let fn_attrs = &input_fn.attrs;

    // --- Hydration Signature Rewrite ---
    // Transforms `Context<T>` to `Context<'info, THydrated<'info>>` to enforce
    // zero-copy pointer safety and lifetime guarantees within the instruction body.
    {
        let mut needs_info_lifetime = false;
        for arg in &mut input_fn.sig.inputs {
            if let syn::FnArg::Typed(pat_type) = arg {
                if let syn::Type::Path(type_path) = &*pat_type.ty {
                    let segments = &type_path.path.segments;
                    let is_context = segments.last().map_or(false, |s| s.ident == "Context");

                    if is_context {
                        let last_segment = segments.last().unwrap();
                        if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                            // Skip any lifetimes to find the type argument
                            let mut type_arg = None;
                            for arg in &args.args {
                                if let syn::GenericArgument::Type(syn::Type::Path(inner_path)) = arg
                                {
                                    type_arg = Some(inner_path);
                                    break;
                                }
                            }

                            if let Some(inner_path) = type_arg {
                                let inner_ident = &inner_path.path.segments.last().unwrap().ident;
                                if !inner_ident.to_string().ends_with("Hydrated") {
                                    needs_info_lifetime = true;
                                    let hydrated_ident =
                                        quote::format_ident!("{}Hydrated", inner_ident);

                                    // Build new arguments: preserve any existing lifetimes if they were there
                                    let mut new_args = syn::punctuated::Punctuated::<
                                        syn::GenericArgument,
                                        syn::Token![,],
                                    >::new();
                                    let mut has_explicit_info = false;
                                    for arg in &args.args {
                                        if let syn::GenericArgument::Lifetime(lt) = arg {
                                            if lt.ident == "info" {
                                                has_explicit_info = true;
                                            }
                                            new_args.push(arg.clone());
                                        }
                                    }
                                    if !has_explicit_info {
                                        new_args.push(syn::parse_quote!('info));
                                    }
                                    new_args.push(syn::parse_quote!(#hydrated_ident<'info>));

                                    let mut new_seg = last_segment.clone();
                                    new_seg.arguments = syn::PathArguments::AngleBracketed(
                                        syn::AngleBracketedGenericArguments {
                                            colon2_token: None,
                                            lt_token: syn::parse_quote!(<),
                                            args: new_args,
                                            gt_token: syn::parse_quote!(>),
                                        },
                                    );
                                    let mut new_path = type_path.path.clone();
                                    *new_path.segments.last_mut().unwrap() = new_seg;
                                    pat_type.ty = Box::new(syn::Type::Path(syn::TypePath {
                                        qself: None,
                                        path: new_path,
                                    }));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Inject <'info> if needed
        if needs_info_lifetime {
            let has_info = input_fn
                .sig
                .generics
                .lifetimes()
                .any(|lt| lt.lifetime.ident == "info");
            if !has_info {
                input_fn.sig.generics.params.push(syn::parse_quote!('info));
            }
        }
    }

    let fn_sig = &input_fn.sig;

    let expanded = quote! {
        #(#fn_attrs)*
        #fn_vis #fn_sig {
            #fn_block
        }
    };

    TokenStream::from(expanded)
}
