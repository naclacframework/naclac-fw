use naclac_lang::prelude::*;

/// Attaches an Oracle external adapter to an existing Asset via
/// `attach_asset_oracle_signed`.
#[derive(Accounts)]
pub struct AttachOracle {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// The external Metaplex Core program to CPI into.
    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn attach_oracle(ctx: Context<AttachOracle>) -> Result {
    let base_address = Address::new_from_array([0u8; 32]);
    // The real processor's `validate_lifecycle_checks` rejects an empty list
    // (`MplCoreError::RequiresLifecycleCheck`), and Oracle adapters may only
    // ever be configured to reject (`flags: 0x4`, `can_reject_only`) per
    // `MplCoreError::OracleCanRejectOnly` — verified in
    // `programs/mpl-core/src/plugins/utils.rs::validate_lifecycle_checks`.
    let lifecycle_checks: &[(HookableLifecycleEventArg, ExternalCheckResultArg)] =
        &[(HookableLifecycleEventArg::Transfer, ExternalCheckResultArg { flags: 0x4 })];
    let base_address_config = None::<ExtraAccountArg>;
    let results_offset: Option<ValidationResultsOffsetArg> = None;

    attach_asset_oracle_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddAssetPluginAccounts {
            asset: ctx.accounts.asset.to_cpi_handle_mut(),
            collection: None,
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        base_address,
        lifecycle_checks,
        base_address_config,
        results_offset,
        &[],
    )?;

    Ok(())
}
