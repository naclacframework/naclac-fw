//! `#[seeds(PREFIX, field, .., bump = field)]` on an `#[account]` struct: the
//! seeds of the PDA that holds this account, named once, from the account's
//! own fields. It generates `with_signer_seeds`, which signs a CPI as that PDA.

use {
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
    syn::{
        parse::{Parse, ParseStream},
        Expr, Ident, ItemStruct, Token,
    },
};

/// The most seeds a PDA takes, bump included; equal to
/// `naclac_core::prelude::MAX_PDA_SEEDS`.
const MAX_SEEDS: usize = 16;

/// A parsed `#[seeds(..)]`: the leading constant, the fields that follow it
/// in order, and the `u8` field holding the bump.
pub(super) struct SeedsSpec {
    prefix: Expr,
    parts: Vec<Ident>,
    bump: Ident,
}

impl Parse for SeedsSpec {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let prefix: Expr = input.parse()?;
        let mut parts = Vec::new();
        let mut bump: Option<Ident> = None;
        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            let name: Ident = input.parse()?;
            if name == "bump" && input.peek(Token![=]) {
                input.parse::<Token![=]>()?;
                let field: Ident = input.parse()?;
                if bump.replace(field).is_some() {
                    return Err(syn::Error::new_spanned(
                        name,
                        "Naclac Error: `#[seeds]` takes one `bump = <field>`.",
                    ));
                }
                continue;
            }
            parts.push(name);
        }
        let Some(bump) = bump else {
            return Err(input.error(
                "Naclac Error: `#[seeds]` needs `bump = <field>`, naming the account's `u8` bump \
                 field, e.g. `#[seeds(SEED, owner, bump = bump)]`.",
            ));
        };
        Ok(SeedsSpec {
            prefix,
            parts,
            bump,
        })
    }
}

/// The `#[seeds(..)]` of `item`, if it has one. More than one is an error.
pub(super) fn parse(item: &ItemStruct) -> syn::Result<Option<SeedsSpec>> {
    let mut found: Option<SeedsSpec> = None;
    for attr in item.attrs.iter().filter(|a| a.path().is_ident("seeds")) {
        let spec = attr.parse_args::<SeedsSpec>()?;
        if found.replace(spec).is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                "Naclac Error: an account takes one `#[seeds(..)]`.",
            ));
        }
    }
    Ok(found)
}

/// The inherent `with_signer_seeds` of `item`: it builds the seed slices from
/// the account's own fields and the stored bump and hands them to a closure as
/// the `&[&[&[u8]]]` every signed CPI takes, on both runtimes. Every seed's
/// bytes come from `SeedPart`, so a field type that cannot be a seed, or one
/// over `MAX_PDA_SEED_LEN` bytes, fails to compile.
pub(super) fn generate(item: &ItemStruct, spec: &SeedsSpec) -> syn::Result<TokenStream> {
    let name = &item.ident;
    let syn::Fields::Named(fields) = &item.fields else {
        return Err(syn::Error::new_spanned(
            &item.ident,
            "Naclac Error: `#[seeds]` needs an account with named fields.",
        ));
    };
    let field_type = |field: &Ident| {
        fields
            .named
            .iter()
            .find(|f| f.ident.as_ref() == Some(field))
            .map(|f| &f.ty)
            .ok_or_else(|| {
                syn::Error::new_spanned(
                    field,
                    format!(
                        "Naclac Error: `#[seeds]` names `{field}`, which is not a field of `{name}`."
                    ),
                )
            })
    };

    let total = spec.parts.len() + 2;
    if total > MAX_SEEDS {
        return Err(syn::Error::new_spanned(
            &spec.prefix,
            format!(
                "Naclac Error: `#[seeds]` makes {total} seeds with the prefix and bump; a PDA takes at most {MAX_SEEDS}."
            ),
        ));
    }

    let mut length_asserts = Vec::new();
    let mut bindings = Vec::new();
    let mut slices = Vec::new();
    for part in &spec.parts {
        let ty = field_type(part)?;
        let binding = format_ident!("__seed_{}", part);
        let message = format!("Naclac Error: seed `{part}` is longer than one PDA seed may be.");
        length_asserts.push(quote! {
            const _: () = ::core::assert!(
                <#ty as naclac_lang::prelude::SeedPart>::LEN
                    <= naclac_lang::prelude::MAX_PDA_SEED_LEN,
                #message
            );
        });
        bindings.push(quote! {
            let #binding = naclac_lang::prelude::SeedPart::seed_part(&{ self.#part });
        });
        slices.push(quote! { ::core::convert::AsRef::<[u8]>::as_ref(&#binding) });
    }
    field_type(&spec.bump)?;

    let prefix = &spec.prefix;
    let bump = &spec.bump;
    let seed_count = total;
    let prefix_message = format!(
        "Naclac Error: the `#[seeds]` prefix of `{name}` is longer than one PDA seed may be."
    );

    let vis = &item.vis;
    let set_name = format_ident!("{}SeedSet", name);
    let part_names = &spec.parts;
    let part_types: Vec<&syn::Type> = spec
        .parts
        .iter()
        .map(|part| field_type(part))
        .collect::<syn::Result<_>>()?;
    let slice_count_with_bump = seed_count;
    let slice_count = seed_count - 1;
    let prefix_name_tokens = match &spec.prefix {
        Expr::Path(path) => match path.path.segments.last() {
            Some(segment) => {
                let text = segment.ident.to_string();
                quote! { Some(#text) }
            }
            None => quote! { None },
        },
        _ => quote! { None },
    };

    Ok(quote! {
        #(#length_asserts)*

        /// The seeds of this account's PDA, built by its `seeds` function.
        #vis struct #set_name {
            __prefix: &'static [u8],
            #( #part_names: <#part_types as naclac_lang::prelude::SeedPart>::Bytes, )*
        }

        impl #set_name {
            /// The prefix and the seed fields, in order.
            #[inline(always)]
            pub fn as_slices(&self) -> [&[u8]; #slice_count] {
                [
                    self.__prefix,
                    #( ::core::convert::AsRef::<[u8]>::as_ref(&self.#part_names), )*
                ]
            }

            /// The same seeds with `bump` as the last one, as a signed CPI takes them.
            #[inline(always)]
            pub fn as_slices_with_bump<'a>(&'a self, bump: &'a [u8; 1]) -> [&'a [u8]; #slice_count_with_bump] {
                [
                    self.__prefix,
                    #( ::core::convert::AsRef::<[u8]>::as_ref(&self.#part_names), )*
                    &bump[..],
                ]
            }
        }

        impl #name {
            /// The seeds of this account's PDA from the values that make them
            /// up, for a field written `seeds = Account::seeds(..)`.
            #[inline(always)]
            pub fn seeds(#( #part_names: #part_types ),*) -> #set_name {
                #set_name {
                    __prefix: ::core::convert::AsRef::<[u8]>::as_ref(&#prefix),
                    #( #part_names: naclac_lang::prelude::SeedPart::seed_part(&#part_names), )*
                }
            }

            #[cfg(feature = "idl-build")]
            #[doc(hidden)]
            pub fn __naclac_idl_prefix_seed() -> naclac_lang::naclac_idl::idl_build_accounts::IdlSeedBuild {
                let prefix: &[u8] = ::core::convert::AsRef::<[u8]>::as_ref(&#prefix);
                let name: Option<&str> = #prefix_name_tokens;
                naclac_lang::naclac_idl::idl_build_accounts::IdlSeedBuild::Const {
                    value: prefix.to_vec(),
                    name: name.map(Into::into),
                    constant: name.map(|n| {
                        naclac_lang::naclac_idl::idl_type::constant_of(n, vec![], &#prefix)
                    }),
                }
            }
        }

        const _: () = ::core::assert!(
            (#prefix).len() <= naclac_lang::prelude::MAX_PDA_SEED_LEN,
            #prefix_message
        );

        impl #name {
            /// Calls `f` with the signer seeds of the PDA holding this
            /// account: the prefix, the named fields in order, and the stored
            /// bump. Pass them to a `*_signed` CPI.
            #[inline(always)]
            pub fn with_signer_seeds<R>(&self, f: impl ::core::ops::FnOnce(&[&[&[u8]]]) -> R) -> R {
                #(#bindings)*
                let __bump: [u8; 1] = [{ self.#bump }];
                let __seeds: [&[u8]; #seed_count] = [
                    ::core::convert::AsRef::<[u8]>::as_ref(&#prefix),
                    #(#slices,)*
                    &__bump,
                ];
                let __signer: [&[&[u8]]; 1] = [&__seeds];
                f(&__signer)
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(source: &str) -> ItemStruct {
        syn::parse_str(source).unwrap()
    }

    fn generated(source: &str) -> String {
        let item = item(source);
        let spec = parse(&item).unwrap().expect("the struct has #[seeds]");
        generate(&item, &spec).unwrap().to_string()
    }

    fn error(source: &str) -> String {
        let item = item(source);
        match parse(&item).and_then(|spec| generate(&item, &spec.expect("#[seeds]"))) {
            Ok(_) => panic!("the seeds must be rejected"),
            Err(error) => error.to_string(),
        }
    }

    #[test]
    fn a_struct_without_seeds_has_none() {
        assert!(parse(&item("struct A { bump: u8 }")).unwrap().is_none());
    }

    #[test]
    fn the_prefix_fields_and_bump_become_one_signer() {
        let code = generated(
            "#[seeds(SEED_POOL, token_a, id, bump = bump)] struct Pool { token_a: Address, id: u64, bump: u8 }",
        );
        assert!(code.contains("with_signer_seeds"), "{code}");
        assert!(code.contains("; 4usize") || code.contains("; 4]"), "{code}");
        assert!(code.contains("__seed_token_a"), "{code}");
        assert!(code.contains("__seed_id"), "{code}");
    }

    #[test]
    fn the_bump_is_required() {
        let message = error("#[seeds(SEED, owner)] struct A { owner: Address, bump: u8 }");
        assert!(message.contains("needs `bump = <field>`"), "{message}");
    }

    #[test]
    fn a_seed_must_name_a_field() {
        let message = error("#[seeds(SEED, nobody, bump = bump)] struct A { bump: u8 }");
        assert!(
            message.contains("`nobody`, which is not a field of `A`"),
            "{message}"
        );
        let message = error("#[seeds(SEED, bump = missing)] struct A { owner: Address }");
        assert!(
            message.contains("`missing`, which is not a field"),
            "{message}"
        );
    }

    #[test]
    fn at_most_sixteen_seeds_with_the_prefix_and_bump() {
        let parts = (0..15).map(|i| format!("p{i}")).collect::<Vec<_>>();
        let fields = parts
            .iter()
            .map(|p| format!("{p}: u8,"))
            .collect::<String>();
        let message = error(&format!(
            "#[seeds(SEED, {}, bump = bump)] struct A {{ {fields} bump: u8 }}",
            parts.join(", ")
        ));
        assert!(message.contains("at most 16"), "{message}");
    }

    #[test]
    fn a_second_seeds_attribute_is_rejected() {
        let item = item("#[seeds(A, bump = bump)] #[seeds(B, bump = bump)] struct S { bump: u8 }");
        let message = parse(&item).err().expect("two attributes").to_string();
        assert!(message.contains("takes one `#[seeds(..)]`"), "{message}");
    }
}
