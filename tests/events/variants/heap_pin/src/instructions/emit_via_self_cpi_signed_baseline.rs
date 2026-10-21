use naclac_lang::prelude::*;
use crate::constants::SEED_EVENT_AUTHORITY;

#[derive(Accounts)]
pub struct EmitViaSelfCpiSignedBaseline {
    /// SAFETY: only its address (against the PDA derived from
    /// `SEED_EVENT_AUTHORITY`) is checked here; it's the CPI target
    /// (`log_event_signed`) that checks `is_signer`, once `invoke_signed`
    /// below has actually marked it as one. Its data is never read.
    #[account(seeds = [SEED_EVENT_AUTHORITY], bump)]
    pub event_authority: AccountInfo,
    /// SAFETY: only its address is checked; its data is never read.
    #[account(address = crate::ID)]
    pub program: AccountInfo,
}

/// `sha256("global:log_event_signed")[..8]`.
const LOG_EVENT_SIGNED_DISCRIMINATOR: [u8; 8] =
    [0xfb, 0xe0, 0x78, 0xcf, 0x4b, 0x40, 0xff, 0xc2];

/// Real, security-correct self-CPI baseline: signs the `event_authority` PDA
/// via `invoke_signed` (matching Anchor's `emit_cpi!`), unlike
/// `emit_via_self_cpi_baseline`'s plain unsigned `invoke`. Measures the true
/// CU cost a spoof-resistant `emit_cpi!` feature would actually pay.
pub fn emit_via_self_cpi_signed_baseline(ctx: Context<EmitViaSelfCpiSignedBaseline>) -> Result {
    let mut data = [0u8; 8 + 8];
    data[..8].copy_from_slice(&LOG_EVENT_SIGNED_DISCRIMINATOR);

    let event_authority_address = *ctx.accounts.event_authority.view.address();
    let accounts = [pinocchio::instruction::InstructionAccount::readonly_signer(
        &event_authority_address,
    )];
    let instruction = pinocchio::instruction::InstructionView {
        program_id: ctx.program_id.as_address(),
        accounts: &accounts,
        data: &data,
    };
    let handles = [ctx.accounts.event_authority.to_cpi_handle()];
    cpi::invoke_signed_pinocchio_handles(
        &instruction,
        &handles,
        &[&[SEED_EVENT_AUTHORITY, &[ctx.bumps.event_authority]]],
    )
}
