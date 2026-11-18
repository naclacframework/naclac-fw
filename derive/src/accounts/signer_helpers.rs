//! `with_<field>_signer` on an Accounts struct: for each field written
//! `seeds = T::seeds(..)`, a method that signs a CPI as that PDA from the same
//! seeds expression the field check evaluates, so a handler never writes the
//! seeds a second time.

use {
    super::ix_args::InstructionArgs,
    crate::accounts::field::{
        checks::{mentions_any_name, typed_seeds_call},
        parser::{ParsedField, PdaSeeds},
    },
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
};

/// Names the generated method uses for its own parameters, which a seed's
/// expression therefore cannot name.
const RESERVED: [&str; 3] = ["self", "bump", "f"];

/// The `with_<field>_signer` methods of an Accounts struct, one per field with
/// a typed `seeds = T::seeds(..)`. Each takes, in declaration order, the
/// `#[instruction(..)]` arguments its seeds name, then the bump (the field's
/// `ctx.bumps` entry) and a closure receiving the accounts again, mutably, and
/// the `&[CpiSigner]` a `*_signed` CPI takes. Every seed value is copied into
/// a local before the closure runs, so the closure may borrow any account,
/// mutably or not. A field gets none when it is optional, takes `seeds::program`
/// (another program's PDA, which this one cannot sign for), or its seeds name
/// `program_id`, an optional field or a reserved name: none of those has a
/// value inside the method.
pub(super) fn signer_helpers(fields: &[ParsedField], args: &InstructionArgs) -> TokenStream {
    let optional: Vec<String> = fields
        .iter()
        .filter(|field| field.is_optional)
        .map(|field| field.ident.to_string())
        .collect();

    let methods = fields.iter().filter_map(|field| {
        let Some(PdaSeeds::Typed(call)) = &field.pda_seed else {
            return None;
        };
        if field.is_optional || field.pda_program.is_some() {
            return None;
        }
        let mentions = |name: &str| {
            call.args
                .iter()
                .any(|arg| mentions_any_name(arg, &[name.to_string()]))
        };
        let mut unavailable = std::iter::once("program_id")
            .chain(RESERVED)
            .chain(optional.iter().map(String::as_str));
        if unavailable.any(mentions) {
            return None;
        }

        let field_bindings = fields
            .iter()
            .filter(|other| mentions(&other.ident.to_string()))
            .map(|other| {
                let name = &other.ident;
                quote! { let #name = &self.#name; }
            });
        let (arg_names, arg_types): (Vec<_>, Vec<_>) = args
            .names
            .iter()
            .zip(&args.types)
            .filter(|(name, _)| mentions(&name.to_string()))
            .unzip();
        let method = format_ident!("with_{}_signer", field.ident);
        let (seed_bindings, seeds_call) = typed_seeds_call(call, &format_ident!("__signer"));
        let doc = format!(
            " Calls `f` with these accounts and the signer seeds of `{}`'s PDA: \
             its `seeds` with `bump`, which is `ctx.bumps.{}`.",
            field.ident, field.ident
        );

        Some(quote! {
            #[doc = #doc]
            #[inline(always)]
            pub fn #method<R>(
                &mut self,
                #( #arg_names: #arg_types, )*
                bump: u8,
                f: impl ::core::ops::FnOnce(&mut Self, &[naclac_lang::prelude::CpiSigner]) -> R,
            ) -> R {
                #(#field_bindings)*
                #seed_bindings
                let __bump: [u8; 1] = [bump];
                let __set = #seeds_call;
                let __seeds = __set.signer_seeds(&__bump);
                let __signer = [__seeds.signer()];
                f(self, &__signer)
            }
        })
    });

    quote! { #(#methods)* }
}

#[cfg(test)]
mod tests {
    use {
        super::signer_helpers,
        crate::accounts::{field::parser::parse_struct_fields, ix_args::InstructionArgs},
    };

    fn helpers(source: &str) -> String {
        let item: syn::ItemStruct = syn::parse_str(source).unwrap();
        let syn::Fields::Named(named) = &item.fields else {
            panic!("a struct with named fields");
        };
        let fields = parse_struct_fields(named).unwrap();
        let args = InstructionArgs::parse(&item.attrs);
        signer_helpers(&fields, &args).to_string()
    }

    #[test]
    fn a_typed_seeds_field_signs_from_the_same_expression_and_the_arguments_it_names() {
        let code = helpers(
            "#[instruction(unused: u8, id: u64)] struct Take { \
             maker: Signer, \
             #[account(mut, seeds = EscrowState::seeds(maker.address(), id), bump = escrow.bump)] \
             escrow: Account<EscrowState> }",
        );
        assert!(code.contains("pub fn with_escrow_signer < R >"), "{code}");
        assert!(
            code.contains("(& mut self , id : u64 , bump : u8 ,"),
            "{code}"
        );
        assert!(code.contains("f (self , & __signer)"), "{code}");
        assert!(!code.contains("unused"), "{code}");
        assert!(code.contains("let maker = & self . maker ;"), "{code}");
        assert!(code.contains("EscrowState :: seeds"), "{code}");
        assert!(code.contains("signer_seeds (& __bump)"), "{code}");
    }

    #[test]
    fn list_seeds_foreign_program_optional_and_unavailable_values_get_no_helper() {
        for field in [
            "#[account(seeds = [b\"x\"], bump)] a: Account<A>",
            "#[account(seeds = A::seeds(), bump, seeds::program = OTHER)] a: Account<A>",
            "#[account(seeds = A::seeds(program_id), bump)] a: Account<A>",
            "#[account(seeds = A::seeds(), bump)] a: Option<Account<A>>",
        ] {
            let code = helpers(&format!("struct S {{ {field} }}"));
            assert!(code.is_empty(), "{field}: {code}");
        }
        let code = helpers(
            "struct S { o: Option<Account<B>>, #[account(seeds = A::seeds(o.address()), bump)] a: Account<A> }",
        );
        assert!(code.is_empty(), "{code}");
    }
}
