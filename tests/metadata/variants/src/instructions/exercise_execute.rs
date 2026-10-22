use naclac_lang::prelude::*;

/// Runs a real CPI (a zero-lamport System Program transfer, chosen to be
/// self-contained without pre-funding any PDA) as the asset's own derived
/// `asset_signer`, via `naclac_metadata::execute_signed`.
#[derive(Accounts)]
pub struct ExerciseExecute {
    #[account(mut)]
    pub asset: Signer,

    /// The asset's own derived `asset_signer` PDA (seeds
    /// `["mpl-core-execute", asset]`, owned by the real Metaplex Core
    /// program) — Core signs internally for this account; naclac-metadata
    /// never derives it itself (see `execute.rs`'s header).
    /// SAFETY: only its address is passed through to the real Metaplex
    /// Core program's own `ExecuteV1` handler, which validates the
    /// derivation itself and force-sets its `is_signer` bit for the inner
    /// CPI — nothing here reads or deserializes it. `mut` because it's
    /// also passed as a writable remaining account for the inner System
    /// transfer — a CPI cannot mark an account writable that the outer
    /// transaction itself didn't already include as writable.
    #[account(mut)]
    pub asset_signer: AccountInfo,

    #[account(mut)]
    pub payer: Signer,

    /// The destination of the inner (zero-lamport) System transfer.
    /// SAFETY: only its pubkey is read — it need not exist beforehand.
    #[account(mut)]
    pub destination: AccountInfo,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn exercise_execute(ctx: Context<ExerciseExecute>) -> Result {
    // Real System Program `Transfer` instruction data: 4-byte LE
    // discriminator (2) + 8-byte LE amount. Amount 0 keeps this
    // self-contained — no need to pre-fund the `asset_signer` PDA.
    let mut inner_data = [0u8; 12];
    inner_data[0..4].copy_from_slice(&2u32.to_le_bytes());

    // `is_signer: false` here — at this outer CPI (this program's own
    // invoke into the real Metaplex Core program), `asset_signer` genuinely
    // isn't a signer yet (no seeds have been applied at this level). Core's
    // own processor force-sets it to `true` internally when it builds its
    // own inner invoke_signed CPI one level down (see this file's own
    // `naclac-metadata::execute.rs` header) — passing `true` here instead
    // would claim a signer privilege this CPI doesn't actually have yet,
    // which the runtime rejects as privilege escalation.
    let remaining_accounts = [
        ExecuteRemainingAccount {
            handle: ctx.accounts.asset_signer.to_cpi_handle(),
            is_signer: false,
            is_writable: true,
        },
        ExecuteRemainingAccount {
            handle: ctx.accounts.destination.to_cpi_handle(),
            is_signer: false,
            is_writable: true,
        },
    ];

    execute_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        ExecuteAssetAccounts {
            asset: ctx.accounts.asset.to_cpi_handle_mut(),
            collection: None,
            asset_signer: ctx.accounts.asset_signer.to_cpi_handle(),
            payer: ExecutePayer::Wallet(ctx.accounts.payer.to_cpi_handle_mut()),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            target_program: ctx.accounts.system_program.to_cpi_handle(),
        },
        &inner_data,
        &remaining_accounts,
        &[],
    )?;

    Ok(())
}
