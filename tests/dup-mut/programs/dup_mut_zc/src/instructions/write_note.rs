use naclac_lang::prelude::*;
use crate::components::Note;
use crate::constants::SEED_NOTE;

// Real runtime exercise of `Span<T>`/`ZcString` (naclac-core/src/wrappers/span.rs):
// `text: ZcString` is a genuine zero-allocation instruction argument — parsed
// via `accounts.rs`'s `is_zc_string` codegen branch directly out of the raw
// instruction-data buffer (length-prefixed: 4-byte LE length + UTF-8 bytes),
// no heap allocation involved at any point. Since `#[component]` fields
// can't themselves be `ZcString` (see `components/note.rs`), the bytes get
// copied into a fixed `[u8; 32]` array for persistence — this instruction is
// what proves the *read* side (`ZcString::as_str()`/`Deref`) produces the
// exact original bytes, not a corrupted or truncated view.
#[derive(Accounts)]
#[instruction(text: ZcString)]
pub struct WriteNote {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init_if_needed,
        payer = payer,
        seeds = [SEED_NOTE],
        bump,
        space = 8 + core::mem::size_of::<Note>()
    )]
    pub note: Account<Note>,

    pub system_program: Program<System>,
}

pub fn write_note(ctx: Context<WriteNote>, text: ZcString) -> Result {
    let bytes = text.as_str().as_bytes();
    require!(bytes.len() <= 32, NaclacError::InvalidInstructionData);

    let note = &mut ctx.accounts.note;
    note.bump = ctx.bumps.note;
    note.len = bytes.len() as u8;
    note.message = [0u8; 32];
    note.message[..bytes.len()].copy_from_slice(bytes);
    Ok(())
}
