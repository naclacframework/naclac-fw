use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct EmitViaSelfCpiFixed8 {
    // Same shape as the other self-CPI/no-CPI/sol_log_data baselines —
    // keeps the outer dispatch/account-validation overhead identical.
    /// SAFETY: only its address is checked; its data is never read.
    #[account(address = crate::ID)]
    pub program: AccountInfo,
}

/// `sha256("global:log_event_fixed_8")[..8]`.
const LOG_EVENT_FIXED_8_DISCRIMINATOR: [u8; 8] =
    [0xcb, 0x61, 0x2c, 0x11, 0xc4, 0xae, 0x11, 0x16];

/// Self-CPI carrying an 8-byte payload with no length prefix — the CU cost
/// a real emit-cpi-for-fixed-events feature would actually pay, as opposed
/// to `emit_via_self_cpi_sized`'s generic dynamic-payload encoding.
pub fn emit_via_self_cpi_fixed_8(ctx: Context<EmitViaSelfCpiFixed8>) -> Result {
    let mut data = [0u8; 8 + 8];
    data[..8].copy_from_slice(&LOG_EVENT_FIXED_8_DISCRIMINATOR);

    let instruction = pinocchio::instruction::InstructionView {
        program_id: ctx.program_id.as_address(),
        accounts: &[],
        data: &data,
    };
    cpi::invoke_signed_pinocchio_handles(&instruction, &[], &[])
}
