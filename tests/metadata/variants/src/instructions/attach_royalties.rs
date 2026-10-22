use naclac_lang::prelude::*;

/// Attaches a Royalties plugin to an existing Asset via `naclac_metadata::attach_royalties_signed`.
#[derive(Accounts)]
pub struct AttachRoyalties {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// The external Metaplex Core program to CPI into.
    /// SAFETY: the real Metaplex Core program — external to this workspace,
    /// so there's no naclac `Program<T>` marker type to constrain it with;
    /// callers are expected to pass the real deployed program ID.
    pub mpl_core_program: AccountInfo,
}

pub fn attach_royalties(ctx: Context<AttachRoyalties>, basis_points: u16) -> Result {
    // Provide a single creator with 100% so `mpl-core`'s `validate_royalties`
    // (which requires creators' percentages to sum to 100) succeeds.
    let single_creator = RoyaltyCreator {
        address: Address::new_from_array([0u8; 32]),
        percentage: 100u8,
    };
    let creators_buf = [single_creator];
    let creators: &[RoyaltyCreator] = &creators_buf;
    let rule_set = RuleSetArg::None;

    attach_royalties_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddAssetPluginAccounts {
            asset: ctx.accounts.asset.to_cpi_handle_mut(),
            collection: None,
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        basis_points,
        creators,
        rule_set,
        &[],
    )?;

    Ok(())
}
