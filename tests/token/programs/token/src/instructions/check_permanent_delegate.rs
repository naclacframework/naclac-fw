use naclac_lang::prelude::*;

/// Reads `PermanentDelegate` off a mint via `get_extension`
/// (`naclac-token/src/extensions.rs`) and asserts the delegate matches the
/// expected value — proves the TLV byte-offset math for this extension type
/// end-to-end.
#[derive(Accounts)]
pub struct CheckPermanentDelegate {
    pub mint: InterfaceAccount<Mint>,
}

pub fn check_permanent_delegate(
    ctx: Context<CheckPermanentDelegate>,
    expected_delegate: Option<Address>,
) -> Result {
    let delegate: PermanentDelegate = ctx.accounts.mint.get_extension()?;

    if delegate.delegate() != expected_delegate {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
