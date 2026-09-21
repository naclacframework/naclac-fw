use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct EmitViaSelfCpiBaseline {
    /// The invoking program's own account. A self-CPI's target program_id is
    /// resolved from the current instruction's own account list, not from
    /// `invoke`'s arguments — so this program must appear here, the same
    /// reason Anchor's `emit_cpi!` requires a `Program<'info, Self>` account.
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
pub fn emit_via_self_cpi_baseline(ctx: Context<EmitViaSelfCpiBaseline>) -> Result {
    // `log_event`'s `data: Vec<u8>` arg is Borsh-decoded, which needs a
    // 4-byte LE length prefix even for an empty Vec — the discriminator
    // alone isn't a valid instruction data layout.
    let mut data = LOG_EVENT_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&0u32.to_le_bytes());

    let ix = solana_program::instruction::Instruction {
        program_id: *ctx.program_id,
        accounts: vec![],
        data,
    };
    invoke(&ix, &[])
}
