use naclac_lang::prelude::*;
use crate::state::Ledger;

// `init` on a keypair-addressed account (no seeds). A client may allocate the
// account first, so `init` also accepts one that already holds zeroed data;
// that account must belong to this program.
#[derive(Accounts)]
pub struct InitKeyed {
    #[account(mut)]
    pub payer: Signer,

    #[account(init, payer = payer, signer)]
    pub ledger: Account<Ledger>,

    pub system_program: Program<System>,
}

pub fn init_keyed(ctx: Context<InitKeyed>) -> Result {
    ctx.accounts.ledger.value += 1;
    Ok(())
}
