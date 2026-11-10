use naclac_lang::prelude::*;
use crate::state::{Entry, Registry};
use crate::constants::SEED_ENTRY;

// `registry.as_ref()` here is a METHOD CALL on the whole zero-copy account
// (via `Account<T>: AsRefByteSlice`, returning the account's own
// address bytes) — not a struct-field path. This is the exact shape
// ZERO_COPY_BORSH_PARITY_AUDIT.md finding #3 broke on (`token_a_mint.as_ref()`
// misrewritten as a `Deref`-based field access instead of falling through to
// the generic `.as_ref()` normalization). Also exercises `init_cpi.rs`'s
// independent seed-binding codegen, since `entry` is being `init`'d with a
// dynamic (non-compile-time-literal) seed.
#[derive(Accounts)]
pub struct InitEntry {
    #[account(mut)]
    pub payer: Signer,

    pub registry: Account<Registry>,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_ENTRY, registry.as_ref()],
        bump
    )]
    pub entry: Account<Entry>,

    pub system_program: Program<System>,
}

pub fn init_entry(ctx: Context<InitEntry>) -> Result {
    let entry = &mut ctx.accounts.entry;
    entry.bump = ctx.bumps.entry;
    entry.value = 0;
    Ok(())
}
