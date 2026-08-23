use naclac_lang::prelude::*;
use crate::components::{Entry, Registry};
use crate::constants::SEED_ENTRY;

// `registry.as_ref()` here is a METHOD CALL on the whole zero-copy account
// (via `Account<T>: AsRefByteSlice`, returning the account's own
// address bytes) — not a struct-field path. This is the exact shape
// ZERO_COPY_BORSH_PARITY_AUDIT.md finding #3 broke on (`token_a_mint.as_ref()`
// misrewritten as a `Deref`-based field access instead of falling through to
// the generic `.as_ref()` normalization). Also exercises `init_cpi.rs`'s
// independent seed-binding codegen, since `entry` is being `init`'d with a
// dynamic (non-compile-time-literal) seed.
//
// Dynamic seeds on `init` cannot use bare `bump` — there's no stored bump to
// read yet, since the account doesn't exist until this instruction creates
// it — so `bump` must be supplied explicitly, computed off-chain by the
// caller via `Address::find_program_address`.
#[derive(Accounts)]
#[instruction(bump: u8)]
pub struct InitEntry {
    #[account(mut)]
    pub payer: Signer,

    pub registry: Account<Registry>,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_ENTRY, registry.as_ref()],
        bump = bump
    )]
    pub entry: Account<Entry>,

    pub system_program: Program<System>,
}

#[instruction]
pub fn init_entry(ctx: Context<InitEntry>, bump: u8) -> Result {
    let entry = &mut ctx.accounts.entry;
    entry.bump = bump;
    entry.value = 0;
    Ok(())
}
