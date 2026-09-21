#![no_std]
use naclac_lang::prelude::*;

declare_id!("FADseVzrrwxbzWN83XK9YXoHULUf4E2LvZDp8F3fKKm8");

// Everything below — the component, both `#[derive(Accounts)]` structs, and
// both instruction handlers — lives directly in this one file, rather than
// the `components/`/`instructions/` split every other test case and example
// uses. That split is a project convention, not something the macros or
// `naclac-syn`'s source discovery require: `parse_workspace_program`
// (naclac-syn/src/lib.rs) recursively walks every `.rs` file under `src/`
// looking for `#[component]`/`#[derive(Accounts)]`/`#[instruction]` items
// wherever they are, so a single `lib.rs` is just as discoverable as a
// deeply split layout. This file (and its matching test) exists to prove
// that claim end-to-end rather than leave it as an unverified assertion —
// IDL generation, client-SDK generation (including the `get_counter_pda`
// helper), and on-chain execution must all work identically to the split
// layout.

pub const SEED_COUNTER: &[u8] = b"counter";

#[component]
pub struct Counter {
    pub bump: u8,
    pub value: u64,
}

#[derive(Accounts)]
pub struct InitCounter {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_COUNTER],
        bump
    )]
    pub counter: Account<Counter>,

    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct IncrementCounter {
    #[account(mut, seeds = [SEED_COUNTER], bump = counter.bump)]
    pub counter: Account<Counter>,
}

#[program]
pub mod single_file_layout {
    pub fn init_counter(ctx: Context<InitCounter>) -> Result {
        ctx.accounts.counter.value = 0;
        Ok(())
    }

    pub fn increment_counter(ctx: Context<IncrementCounter>) -> Result {
        ctx.accounts.counter.value += 1;
        Ok(())
    }
}
