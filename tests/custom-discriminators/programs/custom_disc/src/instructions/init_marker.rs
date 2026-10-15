use naclac_lang::prelude::*;
use crate::components::Marker;
use crate::constants::SEED_MARKER;

#[derive(Accounts)]
pub struct InitMarker {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_MARKER]
    )]
    pub marker: Account<Marker>,

    pub system_program: Program<System>,
}

pub fn init_marker(ctx: Context<InitMarker>) -> Result {
    let marker = &mut ctx.accounts.marker;
    marker.owner = ctx.accounts.payer.address();
    marker.tag = 1;
    Ok(())
}
