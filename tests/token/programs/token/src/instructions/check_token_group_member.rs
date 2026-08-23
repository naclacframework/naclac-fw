use naclac_lang::prelude::*;

/// Reads the `TokenGroupMember` extension off a Token-2022 mint via
/// `InterfaceAccount<Mint>::get_extension` and asserts `mint`/`group` match,
/// proving `initialize_token_group_member`'s CPI actually wrote real
/// on-chain state (not just that the group's `size` counter incremented).
/// Under the pinocchio backend, naclac's own `Address` is a distinct
/// newtype from the real `solana_address::Address`
/// (`spl_token_group_interface::state::TokenGroupMember`'s own field type)
/// — `expected_mint`/`expected_group` go through `Address::as_address()`
/// before comparing, same reasoning as `naclac-token`'s own `ix_addr`
/// helper (`extensions/mod.rs`).
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

    if member.mint != *expected_mint.as_address() || member.group != *expected_group.as_address() {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
