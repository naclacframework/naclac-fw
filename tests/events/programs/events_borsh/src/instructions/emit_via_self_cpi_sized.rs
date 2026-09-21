use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct EmitViaSelfCpiSized {
    // Same shape as the other self-CPI/no-CPI/sol_log_data baselines —
    // keeps the outer dispatch/account-validation overhead identical so the
    // swept `size` is the only variable across the comparison.
    /// SAFETY: only its address is checked; its data is never read.
    #[account(address = crate::ID)]
    pub program: AccountInfo,
}

/// `sha256("global:log_event")[..8]` — must match `log_event`'s own
/// discriminator or this self-CPI silently routes to the wrong handler.
const LOG_EVENT_DISCRIMINATOR: [u8; 8] = [0x05, 0x09, 0x5a, 0x8d, 0xdf, 0x86, 0x39, 0xd9];

/// Self-CPI with a `size`-byte payload — sweeps CU cost against payload size.
pub fn emit_via_self_cpi_sized(ctx: Context<EmitViaSelfCpiSized>, size: u32) -> Result {
    let payload = vec![0u8; size as usize];

    let mut data = LOG_EVENT_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    data.extend_from_slice(&payload);

    let ix = solana_program::instruction::Instruction {
        program_id: *ctx.program_id,
        accounts: vec![],
        data,
    };
    invoke(&ix, &[])
}
