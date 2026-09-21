use naclac_lang::prelude::*;
use crate::errors::FeesError;

#[derive(Accounts)]
pub struct RevokeFeeSharingAuthority {}

/// Revoke Fee Sharing Authority
///
/// confirmed via direct execution against the real bytecode —
/// this instruction always reverts with DeprecatedInstruction, checked
/// before touching any accounts.
pub fn revoke_fee_sharing_authority(_ctx: Context<RevokeFeeSharingAuthority>) -> Result {
    Err(FeesError::DeprecatedInstruction.into())
}
