//! Probe: how a component reaches the IDL's `accounts` list. `Direct` is used as a plain
//! `Account<T>`, `OnlyOptional` only inside `Option<Account<T>>`, `OnlyBoxed` only inside
//! `Box<Account<T>>` (raw zero-copy Solana; Pinocchio has no `Box`), and `Unused` by no
//! instruction at all.

use naclac_lang::prelude::*;

declare_id!("3E9iXab81Dyg7qnXH1psQxWDoQBHQ11H2ctERUbbCdwn");

#[component]
pub struct Direct {
    pub a: u64,
}

#[component]
pub struct OnlyOptional {
    pub b: u64,
}

#[component]
pub struct OnlyBoxed {
    pub c: u64,
}

#[component]
pub struct Unused {
    pub d: u64,
}

#[derive(Accounts)]
pub struct Probe {
    pub signer: Signer,
    pub direct: Account<Direct>,
    pub maybe: Option<Account<OnlyOptional>>,
    pub boxed: Box<Account<OnlyBoxed>>,
}

#[program]
pub mod idlp_probe_accounts_zc {
    pub fn probe(ctx: Context<Probe>) -> Result {
        let _ = ctx;
        Ok(())
    }
}
