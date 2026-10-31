use naclac_lang::prelude::*;
use crate::state::Item;
use crate::constants::SEED_ITEM;
use crate::events::ItemCreated;

#[derive(Accounts)]
pub struct InitItem {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_ITEM]
    )]
    pub item: Account<Item>,

    pub system_program: Program<System>,
}

pub fn init_item(ctx: Context<InitItem>) -> Result {
    let owner = ctx.accounts.payer.address();
    let item = &mut ctx.accounts.item;
    item.owner = owner;
    item.value = 0;
    emit!(ItemCreated { owner, value: 0u8 });
    Ok(())
}
