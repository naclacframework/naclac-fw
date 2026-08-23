use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct EmitViaSelfCpiFixed128 {
    /// SAFETY: only its address is checked; its data is never read.
    #[account(address = crate::ID)]
    pub program: AccountInfo,
}

/// `sha256("global:log_event_fixed_128")[..8]`.
const LOG_EVENT_FIXED_128_DISCRIMINATOR: [u8; 8] =
    [0x95, 0xa8, 0xb0, 0xd9, 0x8c, 0x15, 0x4d, 0xc1];

/// See `emit_via_self_cpi_fixed_8` — same purpose, 128-byte fixed payload.
#[instruction]
pub fn emit_via_self_cpi_fixed_128(ctx: Context<EmitViaSelfCpiFixed128>) -> Result {
    let mut data = [0u8; 8 + 128];
    data[..8].copy_from_slice(&LOG_EVENT_FIXED_128_DISCRIMINATOR);

    let instruction = pinocchio::instruction::InstructionView {
        program_id: ctx.program_id.as_address(),
        accounts: &[],
        data: &data,
    };
    cpi::invoke_signed_pinocchio_handles(&instruction, &[], &[])
}
