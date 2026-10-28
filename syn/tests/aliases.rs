//! A crate-local `type X = T;` is read as `T` in instruction arguments and
//! constants, and an alias whose name is also a declared struct elsewhere in
//! the crate is left as the named type.

use {
    naclac_syn::{aliases::Aliases, crate_constants, modules::crate_modules},
    quote::ToTokens,
    std::{collections::HashMap, fs, path::PathBuf},
};

const LIB_RS: &str = r#"
use naclac_lang::prelude::*;

pub type Amount = u64;
type Names = Vec<Amount>;
pub const FEE: Amount = 5 * 100;

pub mod first {
    pub type Same = u8;
}

pub mod second {
    pub struct Same;
}

#[derive(Accounts)]
pub struct Touch {
    pub signer: Signer,
}

#[program]
pub mod sample {
    pub fn touch(ctx: Context<Touch>, amount: Amount, many: Names, same: Same) -> Result {
        Ok(())
    }
}
"#;

fn sample_crate() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "naclac-syn-aliases-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"sample\"\n").unwrap();
    fs::write(dir.join("src/lib.rs"), LIB_RS).unwrap();
    dir
}

fn resolved_handler_args() -> HashMap<String, String> {
    let dir = sample_crate();
    let mut modules = crate_modules(&dir);
    fs::remove_dir_all(&dir).unwrap();
    Aliases::collect(&modules).resolve_modules(&mut modules);

    let handler = modules
        .iter()
        .flat_map(|module| module.items.iter())
        .find_map(|item| match item {
            syn::Item::Fn(item_fn) if item_fn.sig.ident == "touch" => Some(item_fn),
            _ => None,
        })
        .unwrap();
    handler
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(typed) => {
                let syn::Pat::Ident(ident) = &*typed.pat else {
                    return None;
                };
                Some((
                    ident.ident.to_string(),
                    typed.ty.to_token_stream().to_string().replace(' ', ""),
                ))
            }
            syn::FnArg::Receiver(_) => None,
        })
        .collect()
}

#[test]
fn an_alias_in_an_argument_is_its_target() {
    let args = resolved_handler_args();

    assert_eq!(args["amount"], "u64");
    assert_eq!(args["many"], "Vec<u64>");
}

#[test]
fn an_alias_sharing_a_name_with_a_struct_is_not_resolved() {
    let args = resolved_handler_args();

    assert_eq!(args["same"], "Same");
}

#[test]
fn a_constant_of_an_alias_type_is_evaluated() {
    let dir = sample_crate();
    let constants = crate_constants(&dir);
    fs::remove_dir_all(&dir).unwrap();
    let fee = constants.iter().find(|c| c.name == "FEE").unwrap();

    assert_eq!(fee.value, "500");
}
