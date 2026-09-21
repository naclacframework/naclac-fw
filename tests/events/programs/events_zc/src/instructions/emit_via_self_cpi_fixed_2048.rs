use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct EmitViaSelfCpiFixed2048 {
    /// SAFETY: only its address is checked; its data is never read.
    #[account(address = crate::ID)]
    pub program: AccountInfo,
}

/// `sha256("global:log_event_fixed_2048")[..8]`.
const LOG_EVENT_FIXED_2048_DISCRIMINATOR: [u8; 8] =
    [0x12, 0xdd, 0xe7, 0x66, 0xcd, 0x2e, 0xbf, 0xe7];

/// See `emit_via_self_cpi_fixed_8` — same purpose, 2048-byte fixed payload.
pub fn emit_via_self_cpi_fixed_2048(ctx: Context<EmitViaSelfCpiFixed2048>) -> Result {
    let mut data = [0u8; 8 + 2048];
    data[..8].copy_from_slice(&LOG_EVENT_FIXED_2048_DISCRIMINATOR);

    let ix = solana_program::instruction::Instruction {
        program_id: *ctx.program_id,
        accounts: vec![],
        data: data.to_vec(),
    };
    invoke(&ix, &[])
}
