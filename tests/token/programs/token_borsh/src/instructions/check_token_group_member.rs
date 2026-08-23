use naclac_lang::prelude::*;

/// Reads the `TokenGroupMember` extension off a Token-2022 mint via
/// `InterfaceAccount<Mint>::get_extension` and asserts `mint`/`group` match,
/// proving `initialize_token_group_member`'s CPI actually wrote real
/// on-chain state (not just that the group's `size` counter incremented).
#[derive(Accounts)]
pub struct CheckTokenGroupMember {
    pub member_mint: InterfaceAccount<Mint>,
}

#[instruction]
pub fn check_token_group_member(
    ctx: Context<CheckTokenGroupMember>,
    expected_mint: Address,
    expected_group: Address,
) -> Result {
    let member: spl_token_group_interface::state::TokenGroupMember =
        ctx.accounts.member_mint.get_extension()?;

    if member.mint != expected_mint || member.group != expected_group {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
