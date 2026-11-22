//! Creates a note for an authority.

use {crate::state::Note, naclac_lang::prelude::*};

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct InitNote {
    #[account(mut)]
    pub authority: Signer,

    #[account(
        init,
        payer = authority,
        seeds = Note::seeds(authority.address(), id),
        bump
    )]
    pub note: Account<Note>,

    pub system_program: Program<System>,
}

pub fn init_note(ctx: Context<InitNote>, id: u64) -> Result {
    let note = &mut ctx.accounts.note;
    note.authority = ctx.accounts.authority.address();
    note.id = id;
    note.bump = ctx.bumps.note;
    Ok(())
}
