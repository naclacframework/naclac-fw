use naclac_lang::prelude::*;
use crate::state::Ledger;

// `init_if_needed` on a keypair-addressed account (no seeds): an existing
// account is only accepted when this program owns it, since any program can
// write this program's discriminator into an account it owns.
#[derive(Accounts)]
pub struct InitIfNeededKeyed {
    #[account(mut)]
    pub payer: Signer,

    #[account(init_if_needed, payer = payer, signer)]
    pub ledger: Account<Ledger>,

    pub system_program: Program<System>,
}

pub fn init_if_needed_keyed(ctx: Context<InitIfNeededKeyed>) -> Result {
    ctx.accounts.ledger.value += 1;
    Ok(())
}
