//! Return values (`Result<T>`), a `#[system]` function, `require!`, `msg!` and event emission.

use crate::components::Vault;
use crate::errors::{SecondError, VaultError};
use crate::events::{Basic, Dynamic, DynamicNested};
use crate::types::Pair;
use naclac_lang::prelude::*;

#[system]
pub fn bump_balance(vault: &mut Vault) -> Result<u64> {
    vault.balance += 1;
    Ok(vault.balance)
}

#[derive(Accounts)]
pub struct Returning {
    #[account(mut)]
    pub signer: Signer,
    #[account(mut)]
    pub vault: Box<Account<Vault>>,
}

pub fn returns_value(ctx: Context<Returning>) -> Result<u64> {
    let vault = &mut ctx.accounts.vault;
    bump_balance(vault)
}

pub fn returns_optional(ctx: Context<Returning>) -> Result<Option<u64>> {
    require!(ctx.accounts.vault.balance < 100, SecondError::B);
    Ok(Some(ctx.accounts.vault.balance))
}

pub fn emits(ctx: Context<Returning>) -> Result {
    require!(ctx.accounts.vault.balance < 100, VaultError::Plain);
    emit!(Basic {
        id: 1u64,
        amount: 2i64,
        who: ctx.accounts.signer.address(),
        flag: true,
        small: 3u8,
    });
    emit!(Dynamic {
        head: 1u8,
        items: Vec::new(),
        note: String::from("note"),
        maybe: Some(4u64),
        bytes: Vec::new(),
    });
    let inner = Pair { left: 1, right: 2 };
    emit!(DynamicNested {
        inner,
        many: Vec::new(),
    });
    Ok(())
}
