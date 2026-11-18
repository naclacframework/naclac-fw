//! `#[seeds(PREFIX, seed, ..)]` or `#[seeds(PREFIX, seed, .., bump = field)]`
//! on an `#[account]` struct: the seeds of the PDA that holds this account,
//! named once. A seed is a bare name, the account's field of that name, or
//! `name: Type`, a value the account need not store. With `bump = field`,
//! naming the account's `u8` field that stores the bump, and every seed a
//! stored field, it also generates `with_signer_seeds`, which signs a CPI as
//! that PDA from the account alone.
//!
//! `#[derive(Seeds)]` with `#[seeds(PREFIX, name: Type, ..)]` names the seeds
//! of a PDA that holds no program data (a mint, a token account, an unchecked
//! account): the values are the typed parameters of `seeds(..)`, and there is
//! no stored bump.

use {
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
    syn::{
        parse::{Parse, ParseStream},
        Expr, Ident, ItemStruct, Token, Type,
    },
};

/// The most seeds a PDA takes, bump included; equal to
/// `naclac_core::prelude::MAX_PDA_SEEDS`.
const MAX_SEEDS: usize = 16;

/// Where a `#[seeds]` was written: on an `#[account]`, which then is the only
/// type its seeds may be used with, or on a `#[derive(Seeds)]` type, which
/// may stand for the PDA of any account type.
#[derive(Clone, Copy)]
pub(super) enum Origin {
    Account,
    Standalone,
}

/// One seed after the prefix: the field of the account it is read from, or,
/// for a `#[derive(Seeds)]` type, the `name: Type` it is passed as.
struct NamedSeed {
    name: Ident,
    ty: Option<Type>,
}

/// A parsed `#[seeds(..)]`: the leading constant, the seeds that follow it
/// in order, and the `u8` field holding the bump, if the account stores one.
pub(super) struct SeedsSpec {
    prefix: Expr,
    parts: Vec<NamedSeed>,
    bump: Option<Ident>,
}

impl SeedsSpec {
    /// The field `bump = <field>` names: the one the account's stored bump is
    /// read from and written to.
    pub(super) fn bump_field(&self) -> Option<&Ident> {
        self.bump.as_ref()
    }
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
            let ty = if input.peek(Token![:]) {
                input.parse::<Token![:]>()?;
                Some(input.parse::<Type>()?)
            } else {
                None
            };
            parts.push(NamedSeed { name, ty });
        }
        Ok(SeedsSpec {
            prefix,
            parts,
            bump,
        })
    }
}

/// The `#[seeds(..)]` of `item`, if it has one. More than one is an error.
fn find_spec(item: &ItemStruct) -> syn::Result<Option<SeedsSpec>> {
    let mut found: Option<SeedsSpec> = None;
    for attr in item.attrs.iter().filter(|a| a.path().is_ident("seeds")) {
        let spec = attr.parse_args::<SeedsSpec>()?;
        if found.replace(spec).is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                "Naclac Error: a type takes one `#[seeds(..)]`.",
            ));
        }
    }
    Ok(found)
}

/// The `#[seeds(..)]` of an `#[account]` struct, if it has one. A seed
/// written as a bare name is the account's field of that name; one written
/// `name: Type` is a value passed to `seeds(..)` that the account need not
/// store.
pub(super) fn parse(item: &ItemStruct) -> syn::Result<Option<SeedsSpec>> {
    find_spec(item)
}

/// The expansion of `#[derive(Seeds)]`: the seed set, `seeds` and
/// `__naclac_const_pda` of a type that holds no program data, and the
/// permission to use them for a field of any account type.
pub(super) fn expand_derive(input: TokenStream) -> syn::Result<TokenStream> {
    let item: ItemStruct = syn::parse2(input).map_err(|error| {
        syn::Error::new(
            error.span(),
            "Naclac Error: `#[derive(Seeds)]` goes on a struct.",
        )
    })?;
    let Some(spec) = find_spec(&item)? else {
        return Err(syn::Error::new_spanned(
            &item.ident,
            "Naclac Error: `#[derive(Seeds)]` needs `#[seeds(PREFIX, name: Type, ..)]` on the struct.",
        ));
    };
    if let Some(bump) = &spec.bump {
        return Err(syn::Error::new_spanned(
            bump,
            "Naclac Error: a `Seeds` type holds no data, so it has no stored bump; sign with `seeds(..).signer_seeds(&bump)`.",
        ));
    }
    if let Some(untyped) = spec.parts.iter().find(|part| part.ty.is_none()) {
        return Err(syn::Error::new_spanned(
            &untyped.name,
            format!(
                "Naclac Error: seed `{name}` of a `Seeds` type needs a type, e.g. `{name}: Address`.",
                name = untyped.name
            ),
        ));
    }
    generate(&item, &spec, Origin::Standalone)
}

/// The type of the field `field` of `item`.
fn field_type<'a>(item: &'a ItemStruct, field: &Ident) -> syn::Result<&'a Type> {
    let found = match &item.fields {
        syn::Fields::Named(fields) => fields
            .named
            .iter()
            .find(|f| f.ident.as_ref() == Some(field))
            .map(|f| &f.ty),
        _ => None,
    };
    found.ok_or_else(|| {
        syn::Error::new_spanned(
            field,
            format!(
                "Naclac Error: `#[seeds]` names `{field}`, which is not a field of `{}`.",
                item.ident
            ),
        )
    })
}

/// The type of one seed: the one written after its name, else the type of the
/// field of that name.
fn part_type<'a>(item: &'a ItemStruct, part: &'a NamedSeed) -> syn::Result<&'a Type> {
    match &part.ty {
        Some(ty) => Ok(ty),
        None => field_type(item, &part.name),
    }
}

/// The seed set, `seeds` and `__naclac_const_pda` of `item`, a `SeedsFor`
/// impl (for the account itself, or for any account when `origin` is
/// `Standalone`), and, when
/// `#[seeds]` names a bump field and every seed is a stored field, its
/// inherent `with_signer_seeds`: it builds
/// the seed slices from the account's own fields and the stored bump and hands
/// them to a closure as the `&[CpiSigner]` every signed CPI takes, on both
/// runtimes. Without a bump field the caller signs through
/// `seeds(..).signer_seeds(&bump)`. Every seed's
/// bytes come from `SeedPart`, so a field type that cannot be a seed, or one
/// over `MAX_PDA_SEED_LEN` bytes, fails to compile.
///
/// Each seed is read through `SeedPart::borrow_seed_part` on `addr_of!` of the
/// field, so an address or byte array is borrowed from the account in place
/// and a packed field is never referenced. The pointer comes from `&self`,
/// which outlives every use of the seeds, so it satisfies that method's
/// contract.
pub(super) fn generate(
    item: &ItemStruct,
    spec: &SeedsSpec,
    origin: Origin,
) -> syn::Result<TokenStream> {
    let name = &item.ident;
    let reads_fields = spec.bump.is_some() || spec.parts.iter().any(|part| part.ty.is_none());
    if reads_fields && !matches!(item.fields, syn::Fields::Named(_)) {
        return Err(syn::Error::new_spanned(
            &item.ident,
            "Naclac Error: `#[seeds]` needs an account with named fields.",
        ));
    }

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
    for seed in &spec.parts {
        let part = &seed.name;
        let ty = part_type(item, seed)?;
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
            let #binding = unsafe {
                naclac_lang::prelude::SeedPart::borrow_seed_part(::core::ptr::addr_of!(self.#part))
            };
        });
        slices.push(quote! { ::core::convert::AsRef::<[u8]>::as_ref(&#binding) });
    }
    if let Some(bump) = &spec.bump {
        field_type(item, bump)?;
    }

    let prefix = &spec.prefix;
    let seed_count = total;
    let prefix_message = format!(
        "Naclac Error: the `#[seeds]` prefix of `{name}` is longer than one PDA seed may be."
    );

    let vis = &item.vis;
    let set_name = format_ident!("{}SeedSet", name);
    let part_names: Vec<&Ident> = spec.parts.iter().map(|part| &part.name).collect();
    let part_types: Vec<&Type> = spec
        .parts
        .iter()
        .map(|part| part_type(item, part))
        .collect::<syn::Result<_>>()?;
    let seeds_for = match origin {
        Origin::Account => quote! {
            impl naclac_lang::prelude::SeedsFor<#name> for #name {
                const VALID: bool = true;
            }
        },
        Origin::Standalone => quote! {
            impl<A> naclac_lang::prelude::SeedsFor<A> for #name {
                const VALID: bool = true;
            }
        },
    };
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

    let every_seed_is_stored = spec.parts.iter().all(|part| part.ty.is_none());
    let with_signer_seeds = spec
        .bump
        .as_ref()
        .filter(|_| every_seed_is_stored)
        .map(|bump| {
            quote! {
                impl #name {
                    /// Calls `f` with the signer seeds of the PDA holding this
                    /// account: the prefix, the named fields in order, and the stored
                    /// bump. Pass them to a `*_signed` CPI.
                    #[inline(always)]
                    pub fn with_signer_seeds<R>(
                        &self,
                        f: impl ::core::ops::FnOnce(&[naclac_lang::prelude::CpiSigner]) -> R,
                    ) -> R {
                        #(#bindings)*
                        let __bump: [u8; 1] = [{ self.#bump }];
                        let __seeds = naclac_lang::prelude::SignerSeeds::new([
                            ::core::convert::AsRef::<[u8]>::as_ref(&#prefix),
                            #(#slices,)*
                            &__bump,
                        ]);
                        let __signer = [__seeds.signer()];
                        f(&__signer)
                    }
                }
            }
        });

    Ok(quote! {
        #(#length_asserts)*

        /// The seeds of this account's PDA, built by its `seeds` function. It
        /// borrows the address and byte-array seeds it is built from.
        #vis struct #set_name<'a> {
            __prefix: &'static [u8],
            #( #part_names: <#part_types as naclac_lang::prelude::SeedPart>::Held<'a>, )*
            __borrow: ::core::marker::PhantomData<&'a ()>,
        }

        impl #set_name<'_> {
            /// The prefix and the seed fields, in order.
            #[inline(always)]
            pub fn as_slices(&self) -> [&[u8]; #slice_count] {
                [
                    self.__prefix,
                    #( ::core::convert::AsRef::<[u8]>::as_ref(&self.#part_names), )*
                ]
            }

            /// The same seeds with `bump` as the last one.
            #[inline(always)]
            pub fn as_slices_with_bump<'a>(&'a self, bump: &'a [u8; 1]) -> [&'a [u8]; #slice_count_with_bump] {
                [
                    self.__prefix,
                    #( ::core::convert::AsRef::<[u8]>::as_ref(&self.#part_names), )*
                    &bump[..],
                ]
            }

            /// The same seeds with `bump` as the last one, as the signer seeds
            /// of a signed CPI: pass `&[seeds.signer()]` to a `*_signed` call.
            #[inline(always)]
            pub fn signer_seeds<'a>(
                &'a self,
                bump: &'a [u8; 1],
            ) -> naclac_lang::prelude::SignerSeeds<'a, #slice_count_with_bump> {
                naclac_lang::prelude::SignerSeeds::new(self.as_slices_with_bump(bump))
            }
        }

        impl #name {
            /// The seeds of this account's PDA from references to the values
            /// that make them up. A field writes `seeds = Account::seeds(..)`
            /// with the values and the derive passes references to them.
            #[inline(always)]
            pub fn seeds<'a>(#( #part_names: &'a #part_types ),*) -> #set_name<'a> {
                #set_name {
                    __prefix: ::core::convert::AsRef::<[u8]>::as_ref(&#prefix),
                    #( #part_names: naclac_lang::prelude::SeedPart::hold(#part_names), )*
                    __borrow: ::core::marker::PhantomData,
                }
            }

            /// The PDA address and bump of these seeds under `__program`,
            /// for a `const` the compiler evaluates. `#[derive(Accounts)]`
            /// calls it for a field whose `seeds = Account::seeds(..)` names
            /// no runtime value.
            #[doc(hidden)]
            pub const fn __naclac_const_pda(
                #( #part_names: #part_types, )*
                __program: &naclac_lang::prelude::Address,
            ) -> (naclac_lang::prelude::Address, u8) {
                naclac_lang::prelude::find_program_address_const(
                    &[
                        &naclac_lang::prelude::ConstSeed(#prefix).bytes(),
                        #( &naclac_lang::prelude::ConstSeed(#part_names).bytes(), )*
                    ],
                    __program,
                )
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

        #seeds_for

        #with_signer_seeds
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
        generate(&item, &spec, Origin::Account).unwrap().to_string()
    }

    fn error(source: &str) -> String {
        let item = item(source);
        match parse(&item)
            .and_then(|spec| generate(&item, &spec.expect("#[seeds]"), Origin::Account))
        {
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
    fn the_seed_set_borrows_the_values_it_is_built_from() {
        let code = generated(
            "#[seeds(SEED_POOL, token_a, id, bump = bump)] struct Pool { token_a: Address, id: u64, bump: u8 }",
        );
        assert!(code.contains("token_a : & 'a Address"), "{code}");
        assert!(code.contains("id : & 'a u64"), "{code}");
        assert!(code.contains("SeedPart :: hold (token_a)"), "{code}");
        assert!(code.contains("PoolSeedSet < 'a >"), "{code}");
    }

    #[test]
    fn each_seed_is_borrowed_from_its_field_in_place() {
        let code = generated(
            "#[seeds(SEED_POOL, token_a, id, bump = bump)] struct Pool { token_a: Address, id: u64, bump: u8 }",
        );
        assert!(
            code.contains("borrow_seed_part (:: core :: ptr :: addr_of ! (self . token_a))"),
            "{code}"
        );
        assert!(
            code.contains("borrow_seed_part (:: core :: ptr :: addr_of ! (self . id))"),
            "{code}"
        );
        assert!(!code.contains("seed_part (& { self"), "{code}");
    }

    #[test]
    fn a_const_fn_derives_the_pda_from_the_prefix_and_the_values() {
        let code = generated(
            "#[seeds(SEED_POOL, token_a, id, bump = bump)] struct Pool { token_a: Address, id: u64, bump: u8 }",
        );
        assert!(
            code.contains("const fn __naclac_const_pda (token_a : Address , id : u64 , __program : & naclac_lang :: prelude :: Address ,)"),
            "{code}"
        );
        assert!(code.contains("ConstSeed (SEED_POOL) . bytes ()"), "{code}");
        assert!(code.contains("ConstSeed (token_a) . bytes ()"), "{code}");
        assert!(code.contains("ConstSeed (id) . bytes ()"), "{code}");
    }

    #[test]
    fn signing_from_the_account_needs_a_named_bump_field() {
        let with =
            generated("#[seeds(SEED, owner, bump = bump)] struct A { owner: Address, bump: u8 }");
        assert!(with.contains("with_signer_seeds"), "{with}");
        let without = generated("#[seeds(SEED, owner)] struct A { owner: Address }");
        assert!(!without.contains("with_signer_seeds"), "{without}");
        for code in [&with, &without] {
            assert!(code.contains("fn signer_seeds"), "{code}");
            assert!(code.contains("fn __naclac_const_pda"), "{code}");
        }
    }

    #[test]
    fn the_bump_field_is_the_one_the_attribute_names() {
        let named = item("#[seeds(SEED, bump = stored)] struct A { stored: u8 }");
        let spec = parse(&named).unwrap().unwrap();
        assert_eq!(
            spec.bump_field().map(ToString::to_string).as_deref(),
            Some("stored")
        );
        let unnamed = item("#[seeds(SEED)] struct A { bump: u8 }");
        assert!(parse(&unnamed).unwrap().unwrap().bump_field().is_none());
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

    fn standalone(source: &str) -> syn::Result<String> {
        expand_derive(source.parse().unwrap()).map(|tokens| tokens.to_string())
    }

    #[test]
    fn a_seeds_type_takes_its_values_as_typed_parameters_and_never_signs_from_data() {
        let code =
            standalone("#[seeds(SEED_VAULT, owner: Address, index: u32)] pub struct VaultSeeds;")
                .unwrap();
        assert!(code.contains("owner : & 'a Address"), "{code}");
        assert!(code.contains("index : & 'a u32"), "{code}");
        assert!(
            code.contains("const fn __naclac_const_pda (owner : Address , index : u32 ,"),
            "{code}"
        );
        assert!(!code.contains("with_signer_seeds"), "{code}");
        assert!(code.contains("fn signer_seeds"), "{code}");
    }

    #[test]
    fn a_seeds_type_may_name_the_seeds_of_any_account_but_an_account_only_its_own() {
        let standalone_code = standalone("#[seeds(SEED, key: Address)] struct S;").unwrap();
        assert!(
            standalone_code.contains("SeedsFor < A > for S"),
            "{standalone_code}"
        );
        let account_code = generated("#[seeds(SEED, key)] struct Pool { key: Address }");
        assert!(
            account_code.contains("SeedsFor < Pool > for Pool"),
            "{account_code}"
        );
        assert!(!account_code.contains("SeedsFor < A >"), "{account_code}");
    }

    #[test]
    fn a_seeds_type_is_checked_for_what_only_an_account_may_have() {
        let message = standalone("#[seeds(SEED, key)] struct S;")
            .unwrap_err()
            .to_string();
        assert!(
            message.contains("`key` of a `Seeds` type needs a type"),
            "{message}"
        );
        let message = standalone("#[seeds(SEED, key: Address, bump = b)] struct S;")
            .unwrap_err()
            .to_string();
        assert!(message.contains("has no stored bump"), "{message}");
        let message = standalone("struct S;").unwrap_err().to_string();
        assert!(
            message.contains("needs `#[seeds(PREFIX, name: Type, ..)]`"),
            "{message}"
        );
        let message = standalone("enum S { A }").unwrap_err().to_string();
        assert!(message.contains("goes on a struct"), "{message}");
    }

    #[test]
    fn an_account_takes_an_unstored_seed_as_a_parameter_and_then_cannot_sign_from_its_data() {
        let code = generated(
            "#[seeds(SEED, maker, id: u64, bump = bump)] struct Escrow { maker: Address, bump: u8 }",
        );
        assert!(code.contains("maker : & 'a Address"), "{code}");
        assert!(code.contains("id : & 'a u64"), "{code}");
        assert!(
            code.contains("const fn __naclac_const_pda (maker : Address , id : u64 ,"),
            "{code}"
        );
        assert!(!code.contains("with_signer_seeds"), "{code}");
        assert!(code.contains("fn signer_seeds"), "{code}");
        assert!(code.contains("SeedsFor < Escrow > for Escrow"), "{code}");

        let item = item("#[seeds(SEED, id: u64, bump = missing)] struct A { other: u8 }");
        let message = parse(&item)
            .and_then(|spec| generate(&item, &spec.unwrap(), Origin::Account))
            .expect_err("the bump names no field")
            .to_string();
        assert!(
            message.contains("`missing`, which is not a field"),
            "{message}"
        );
    }

    #[test]
    fn a_seeds_type_still_takes_at_most_sixteen_seeds() {
        let parts = (0..15)
            .map(|i| format!("p{i}: u8"))
            .collect::<Vec<_>>()
            .join(", ");
        let message = standalone(&format!("#[seeds(SEED, {parts})] struct S;"))
            .unwrap_err()
            .to_string();
        assert!(message.contains("at most 16"), "{message}");
    }

    #[test]
    fn a_second_seeds_attribute_is_rejected() {
        let item = item("#[seeds(A, bump = bump)] #[seeds(B, bump = bump)] struct S { bump: u8 }");
        let message = parse(&item).err().expect("two attributes").to_string();
        assert!(message.contains("takes one `#[seeds(..)]`"), "{message}");
    }
}
