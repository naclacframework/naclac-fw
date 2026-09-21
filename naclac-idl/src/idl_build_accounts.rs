//! Intermediate representation for `#[derive(Accounts)]`'s `idl-build` output.
//!
//! `accounts.rs` can resolve everything about an `Accounts` struct's fields
//! locally at its own macro-expansion time except one thing: an
//! `Account`-kind PDA seed's subfield type (e.g. `registry.bump`'s `u8`)
//! requires the *backing component's* real field types, which a single
//! derive-macro invocation has no visibility into — it only ever sees its
//! own struct's tokens. So `accounts.rs` emits the account's bare type name
//! instead (`IdlSeedBuild::Account.account_type`, e.g. `"Registry"` — always
//! locally derivable from the field's own `Account<'info, Registry>` generic
//! parameter), and `#[program]`'s assembler — which by the time it runs has
//! already collected every referenced component's real fields via
//! `idl_build::NaclacIdlBuild::create_type()` — resolves the subfield's real
//! type by looking it up in that already-assembled map, via [`resolve`].

use crate::{IdlAccount, IdlPda, IdlSeed, IdlTypeDef};
use std::collections::BTreeMap;

pub struct IdlAccountBuild {
    pub name: String,
    pub docs: Vec<String>,
    pub writable: bool,
    pub signer: bool,
    pub optional: bool,
    pub pda: Option<IdlPdaBuild>,
    pub address: Option<String>,
}

pub struct IdlPdaBuild {
    pub seeds: Vec<IdlSeedBuild>,
    pub program: Option<IdlSeedBuild>,
}

pub enum IdlSeedBuild {
    Const {
        value: Vec<u8>,
        name: Option<String>,
    },
    Arg {
        path: String,
    },
    Account {
        path: String,
        account_type: Option<String>,
    },
}

impl IdlSeedBuild {
    pub fn resolve(self, types: &BTreeMap<String, IdlTypeDef>) -> IdlSeed {
        match self {
            IdlSeedBuild::Const { value, name } => IdlSeed::Const { value, name },
            IdlSeedBuild::Arg { path } => IdlSeed::Arg { path },
            IdlSeedBuild::Account { path, account_type } => {
                let field_type = account_type.and_then(|ty_name| {
                    let field_name = path.split_once('.')?.1;
                    let IdlTypeDef::Struct { fields } = types.get(&ty_name)? else {
                        return None;
                    };
                    fields
                        .iter()
                        .find(|f| f.name == field_name)
                        .and_then(|f| f.ty.as_str().map(str::to_string))
                });
                IdlSeed::Account { path, field_type }
            }
        }
    }
}

impl IdlAccountBuild {
    pub fn resolve(self, types: &BTreeMap<String, IdlTypeDef>) -> IdlAccount {
        IdlAccount {
            name: self.name,
            docs: self.docs,
            writable: self.writable,
            signer: self.signer,
            optional: self.optional,
            pda: self.pda.map(|p| IdlPda {
                seeds: p.seeds.into_iter().map(|s| s.resolve(types)).collect(),
                program: p.program.map(|s| s.resolve(types)),
            }),
            address: self.address,
        }
    }
}
