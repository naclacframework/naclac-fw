//! `SerializedBounds` for a Borsh or Wincode `#[defined_type]`.
//!
//! The impl lets an `#[account]` field of the type resolve its maximum
//! serialized size and length-limit check through the compiler, and is built
//! from the same `naclac_syn::bounds` generator as the account's own fields.

use {
    naclac_syn::bounds::{field_bound, max_len_values, Problem},
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
    syn::{Data, DeriveInput, Fields},
};

/// Removes every `#[max_len]` from the fields of `data`. They are read from the
/// unmodified definition by [`generate`]; no derive recognises them.
pub(crate) fn strip_max_len(data: &mut Data) {
    let strip = |fields: &mut Fields| {
        for field in fields.iter_mut() {
            field.attrs.retain(|attr| !attr.path().is_ident("max_len"));
        }
    };
    match data {
        Data::Struct(data) => strip(&mut data.fields),
        Data::Enum(data) => data
            .variants
            .iter_mut()
            .for_each(|variant| strip(&mut variant.fields)),
        Data::Union(_) => {}
    }
}

struct FieldsBound {
    size: TokenStream,
    checks: Vec<TokenStream>,
    patterns: Vec<TokenStream>,
    unbounded: Option<String>,
}

fn fields_bound(
    fields: &Fields,
    access: impl Fn(usize, &syn::Field) -> TokenStream,
    core: &syn::Path,
    aliases: &naclac_syn::aliases::Aliases,
) -> syn::Result<FieldsBound> {
    let mut sizes = Vec::new();
    let mut checks = Vec::new();
    let mut patterns = Vec::new();
    let mut unbounded = None;
    for (position, field) in fields.iter().enumerate() {
        let limits = max_len_values(&field.attrs)?;
        let value = access(position, field);
        let name = field
            .ident
            .as_ref()
            .map_or_else(|| position.to_string(), syn::Ident::to_string);
        match field_bound(&aliases.resolve(&field.ty), &limits, &value, core) {
            Ok(bound) => {
                sizes.push(bound.size);
                checks.push(bound.check);
                patterns.push(match &field.ident {
                    Some(ident) => quote! { #ident },
                    None => {
                        let binding = format_ident!("__f{}", position);
                        quote! { #binding }
                    }
                });
            }
            Err(Problem::Unbounded) => {
                unbounded.get_or_insert(name);
                patterns.push(match &field.ident {
                    Some(ident) => quote! { #ident: _ },
                    None => quote! { _ },
                });
            }
            Err(Problem::Invalid(err)) => return Err(err),
        }
    }
    Ok(FieldsBound {
        size: quote! { 0usize #(+ #sizes)* },
        checks,
        patterns,
        unbounded,
    })
}

fn unbounded_size(type_name: &syn::Ident, field: &str) -> TokenStream {
    let message = format!(
        "`{type_name}` has a `Vec`/`String` field `{field}` with no `#[max_len(N)]`, so its \
         maximum size is unknown: add `#[max_len(N)]` to the field, or give the account an \
         explicit `space = ..`"
    );
    quote! { panic!(#message) }
}

/// The `SerializedBounds` impl of `input`, or nothing for a generic type
/// (whose size depends on its arguments). A struct's maximum size is the sum
/// of its fields; an enum's is one tag byte plus its largest variant. A field
/// without the `#[max_len]` it needs makes `MAX_SIZE` fail to compile where it
/// is used.
pub(crate) fn generate(input: &DeriveInput, cfg: &TokenStream) -> syn::Result<TokenStream> {
    if !input.generics.params.is_empty() {
        return Ok(quote! {});
    }
    let name = &input.ident;
    let core: syn::Path = syn::parse_quote!(naclac_lang::prelude);
    let aliases = crate::crate_aliases();

    let (max_size, within_bounds) = match &input.data {
        Data::Struct(data) => {
            let bound = fields_bound(
                &data.fields,
                |position, field| match &field.ident {
                    Some(ident) => quote! { &self.#ident },
                    None => {
                        let index = syn::Index::from(position);
                        quote! { &self.#index }
                    }
                },
                &core,
                &aliases,
            )?;
            let size = match &bound.unbounded {
                Some(field) => unbounded_size(name, field),
                None => bound.size,
            };
            let checks = &bound.checks;
            (size, quote! { true #(&& #checks)* })
        }
        Data::Enum(data) => {
            let mut sizes = Vec::new();
            let mut arms = Vec::new();
            let mut unbounded = None;
            for variant in &data.variants {
                let variant_name = &variant.ident;
                let bound = fields_bound(
                    &variant.fields,
                    |position, field| match &field.ident {
                        Some(ident) => quote! { #ident },
                        None => {
                            let binding = format_ident!("__f{}", position);
                            quote! { #binding }
                        }
                    },
                    &core,
                    &aliases,
                )?;
                if unbounded.is_none() {
                    unbounded = bound
                        .unbounded
                        .as_ref()
                        .map(|field| unbounded_size(name, field));
                }
                sizes.push(bound.size);
                let checks = &bound.checks;
                let patterns = &bound.patterns;
                let pattern = match &variant.fields {
                    Fields::Named(_) => quote! { Self::#variant_name { #(#patterns),* } },
                    Fields::Unnamed(_) => quote! { Self::#variant_name ( #(#patterns),* ) },
                    Fields::Unit => quote! { Self::#variant_name },
                };
                arms.push(quote! { #pattern => true #(&& #checks)* });
            }
            let size = unbounded.unwrap_or_else(|| {
                quote! {
                    {
                        let mut largest = 0usize;
                        #({
                            let variant_size: usize = #sizes;
                            if variant_size > largest {
                                largest = variant_size;
                            }
                        })*
                        1usize + largest
                    }
                }
            });
            let within = if arms.is_empty() {
                quote! { true }
            } else {
                quote! { match self { #(#arms),* } }
            };
            (size, within)
        }
        Data::Union(_) => return Ok(quote! {}),
    };

    Ok(quote! {
        #cfg
        impl #core::SerializedBounds for #name {
            const MAX_SIZE: usize = #max_size;

            fn within_bounds(&self) -> bool {
                #within_bounds
            }
        }
    })
}
