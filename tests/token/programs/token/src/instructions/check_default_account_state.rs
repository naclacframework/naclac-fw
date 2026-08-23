use naclac_lang::prelude::*;

/// Reads the `DefaultAccountState` extension off a Token-2022 mint via
/// `InterfaceAccount<Mint>::get_extension` and asserts it matches the
/// expected raw `AccountState` byte — the only way to prove the TLV
/// byte-offset math for this extension is correct end-to-end, not just that
/// it compiles.
#[derive(Accounts)]
pub struct CheckDefaultAccountState {
    pub mint: InterfaceAccount<Mint>,
}

#[instruction]
pub fn check_default_account_state(
    ctx: Context<CheckDefaultAccountState>,
    expected_state: u8,
) -> Result {
    let config: DefaultAccountState = ctx.accounts.mint.get_extension()?;

    if config.state() != expected_state {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
