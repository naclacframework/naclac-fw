use naclac_lang::prelude::*;
use crate::components::Thing;
use crate::constants::SEED_THING_A;

#[derive(Accounts)]
pub struct InitThingA {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_THING_A]
    )]
    pub thing_a: Account<Thing>,

    pub system_program: Program<System>,
}

pub fn init_thing_a(ctx: Context<InitThingA>) -> Result {
    let thing_a = &mut ctx.accounts.thing_a;
    thing_a.value = 0;
    Ok(())
}
