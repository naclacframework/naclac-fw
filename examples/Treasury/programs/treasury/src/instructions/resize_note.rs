//! Resizes a note to a size the caller chooses, paid for by its authority.

use {crate::state::Note, naclac_lang::prelude::*};

#[derive(Accounts)]
#[instruction(new_space: u64)]
pub struct ResizeNote {
    #[account(mut)]
    pub authority: Signer,

    #[account(
        mut,
        authority = authority,
        realloc = new_space as usize,
        realloc::payer = authority
    )]
    pub note: Account<Note>,

    pub system_program: Program<System>,
}

pub fn resize_note(_ctx: Context<ResizeNote>, _new_space: u64) -> Result {
    Ok(())
}
