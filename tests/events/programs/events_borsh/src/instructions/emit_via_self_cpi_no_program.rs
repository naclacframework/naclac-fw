use naclac_lang::prelude::*;
use crate::constants::SEED_EVENT_AUTHORITY;

/// Same as `emit_via_self_cpi_signed_baseline` but with no `program` account:
/// proves a self-CPI needs only the signing `event_authority`.
#[derive(Accounts)]
pub struct EmitViaSelfCpiNoProgram {
    /// SAFETY: only its address (against the PDA derived from
    /// `SEED_EVENT_AUTHORITY`) is checked here; it's the CPI target
    /// (`log_event_signed`) that checks `is_signer`, once `invoke_signed`
    /// below has actually marked it as one. Its data is never read.
    #[account(seeds = [SEED_EVENT_AUTHORITY], bump)]
    pub event_authority: AccountInfo,
}

/// `sha256("global:log_event_signed")[..8]`.
const LOG_EVENT_SIGNED_DISCRIMINATOR: [u8; 8] =
    [0xfb, 0xe0, 0x78, 0xcf, 0x4b, 0x40, 0xff, 0xc2];

pub fn emit_via_self_cpi_no_program(ctx: Context<EmitViaSelfCpiNoProgram>) -> Result {
    let mut data = [0u8; 8 + 8];
    data[..8].copy_from_slice(&LOG_EVENT_SIGNED_DISCRIMINATOR);

    let ix = solana_instruction::Instruction {
        program_id: *ctx.program_id,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(
            ctx.accounts.event_authority.address(),
            true,
        )],
        data: data.to_vec(),
    };
    cpi::invoke_signed(
        &ix,
        &[ctx.accounts.event_authority.to_cpi_handle()],
        &[&[SEED_EVENT_AUTHORITY, &[ctx.bumps.event_authority]]],
    )
}
