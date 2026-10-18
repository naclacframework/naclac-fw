//! Source-parsing IDL route: a crate-local `type X = T;` is read as `T` in
//! instruction arguments and constants, and an alias whose name is also a
//! declared struct elsewhere in the crate is left as the named type.

use {naclac_syn::parse_workspace_program, serde_json::json, std::fs};

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

fn sample_program() -> naclac_syn::types::NaclacProgram {
    let dir = std::env::temp_dir().join(format!(
        "naclac-syn-aliases-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/lib.rs"), LIB_RS).unwrap();
    let program = parse_workspace_program(&dir, "sample");
    fs::remove_dir_all(&dir).unwrap();
    program
}

#[test]
fn an_alias_in_an_argument_is_its_target() {
    let program = sample_program();
    let args = &program.instructions[0].args;
    let ty = |name: &str| args.iter().find(|a| a.name == name).unwrap().ty.clone();

    assert_eq!(ty("amount"), json!("u64"));
    assert_eq!(ty("many"), json!({ "vec": "u64" }));
}

#[test]
fn an_alias_sharing_a_name_with_a_struct_is_not_resolved() {
    let program = sample_program();
    let args = &program.instructions[0].args;
    let same = args.iter().find(|a| a.name == "same").unwrap();

    assert_eq!(same.ty, json!({ "defined": "Same" }));
}

#[test]
fn a_constant_of_an_alias_type_is_evaluated() {
    let program = sample_program();
    let fee = program.constants.iter().find(|c| c.name == "FEE").unwrap();

    assert_eq!(fee.value, "500");
}
