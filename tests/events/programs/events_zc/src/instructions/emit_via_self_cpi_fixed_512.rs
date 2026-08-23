use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct EmitViaSelfCpiFixed512 {
    /// SAFETY: only its address is checked; its data is never read.
    #[account(address = crate::ID)]
    pub program: AccountInfo,
}

/// `sha256("global:log_event_fixed_512")[..8]`.
const LOG_EVENT_FIXED_512_DISCRIMINATOR: [u8; 8] =
    [0x86, 0x68, 0xbe, 0xb5, 0xea, 0x9b, 0x8e, 0xbf];

/// See `emit_via_self_cpi_fixed_8` — same purpose, 512-byte fixed payload.
#[instruction]
pub fn emit_via_self_cpi_fixed_512(ctx: Context<EmitViaSelfCpiFixed512>) -> Result {
    let mut data = [0u8; 8 + 512];
    data[..8].copy_from_slice(&LOG_EVENT_FIXED_512_DISCRIMINATOR);

    let ix = solana_program::instruction::Instruction {
        program_id: *ctx.program_id,
        accounts: vec![],
        data: data.to_vec(),
    };
    invoke(&ix, &[])
}
