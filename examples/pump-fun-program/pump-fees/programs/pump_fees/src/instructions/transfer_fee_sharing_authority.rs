use naclac_lang::prelude::*;
use crate::errors::FeesError;

#[derive(Accounts)]
pub struct TransferFeeSharingAuthority {}

/// Transfer Fee Sharing Authority
///
/// confirmed via direct execution against the real bytecode —
/// this instruction always reverts with DeprecatedInstruction, checked
/// before touching any accounts.
#[instruction]
pub fn transfer_fee_sharing_authority(_ctx: Context<TransferFeeSharingAuthority>) -> Result {
    Err(FeesError::DeprecatedInstruction.into())
}
