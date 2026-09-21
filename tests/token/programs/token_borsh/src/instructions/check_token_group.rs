use naclac_lang::prelude::*;

/// Reads the `TokenGroup` extension off a Token-2022 mint via
/// `InterfaceAccount<Mint>::get_extension` (now that
/// `spl_token_group_interface::state::TokenGroup` implements naclac's own
/// `Extension` trait directly — see `token_group.rs`'s own doc comment) and
/// asserts `size`/`max_size` match, proving `initialize_token_group`'s CPI
/// actually wrote real on-chain state.
#[derive(Accounts)]
pub struct CheckTokenGroup {
    pub mint: InterfaceAccount<Mint>,
}

pub fn check_token_group(
    ctx: Context<CheckTokenGroup>,
    expected_size: u64,
    expected_max_size: u64,
) -> Result {
    let group: spl_token_group_interface::state::TokenGroup = ctx.accounts.mint.get_extension()?;

    if u64::from(group.size) != expected_size || u64::from(group.max_size) != expected_max_size {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
