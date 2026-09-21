use naclac_lang::prelude::*;

/// Reads the `GroupMemberPointer` extension off a Token-2022 mint via
/// `InterfaceAccount<Mint>::get_extension` and asserts the authority and
/// member address match the expected values — the only way to prove the
/// TLV byte-offset math is correct end-to-end, not just that it compiles.
#[derive(Accounts)]
pub struct CheckGroupMemberPointer {
    pub mint: InterfaceAccount<Mint>,
}

pub fn check_group_member_pointer(
    ctx: Context<CheckGroupMemberPointer>,
    expected_authority: Option<Address>,
    expected_member_address: Option<Address>,
) -> Result {
    let config: GroupMemberPointer = ctx.accounts.mint.get_extension()?;

    if config.authority() != expected_authority
        || config.member_address() != expected_member_address
    {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
