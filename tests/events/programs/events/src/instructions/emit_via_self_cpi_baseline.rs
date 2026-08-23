use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct EmitViaSelfCpiBaseline {
    // Same shape as `NoCpiBaseline` — kept identical so the outer dispatch/
    // account-validation overhead matches exactly, isolating the CPI
    // mechanism itself as the sole variable between the two.
    /// SAFETY: only its address is checked; its data is never read.
    #[account(address = crate::ID)]
    pub program: AccountInfo,
}

/// `sha256("global:log_event")[..8]` — must match `log_event`'s own
/// discriminator (naclac-macros/src/program.rs's `global:<fn_name>` scheme)
/// or this self-CPI silently routes to the wrong handler.
const LOG_EVENT_DISCRIMINATOR: [u8; 8] = [0x05, 0x09, 0x5a, 0x8d, 0xdf, 0x86, 0x39, 0xd9];

/// Pure self-CPI baseline: invokes `log_event` with zero-length payload and
/// zero accounts, isolating the fixed CU cost of a self-CPI from any
/// per-byte payload cost.
#[instruction]
pub fn emit_via_self_cpi_baseline(ctx: Context<EmitViaSelfCpiBaseline>) -> Result {
    // `log_event`'s `data: Vec<u8>` arg is length-prefixed (4-byte LE),
    // even zero-copy mode's heap-`Vec<u8>` decode path reads that same
    // prefix — the discriminator alone isn't a valid instruction layout.
    let mut data = [0u8; 12];
    data[..8].copy_from_slice(&LOG_EVENT_DISCRIMINATOR);
    data[8..12].copy_from_slice(&0u32.to_le_bytes());

    let instruction = pinocchio::instruction::InstructionView {
        program_id: ctx.program_id.as_address(),
        accounts: &[],
        data: &data,
    };
    cpi::invoke_signed_pinocchio_handles(&instruction, &[], &[])
}
