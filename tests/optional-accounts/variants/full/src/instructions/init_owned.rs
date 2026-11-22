use {
    crate::{constants::SEED_OWNED, state::Owned},
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitOwned {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_OWNED]
    )]
    pub owned: Account<Owned>,

    pub system_program: Program<System>,
}

pub fn init_owned(ctx: Context<InitOwned>) -> Result {
    let owned = &mut ctx.accounts.owned;
    owned.authority = ctx.accounts.payer.address();
    owned.value = 0;
    Ok(())
}
