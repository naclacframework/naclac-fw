use naclac_lang::prelude::*;
use crate::components::launch_record::LaunchRecord;

#[system]
pub fn process_launch_record(
    launch_record: &mut LaunchRecord,
    creator: Address,
    mint: Address,
    amount_token: u64,
    amount_quote: u64,
) -> Result<()> {
    launch_record.creator = creator;
    launch_record.mint = mint;
    launch_record.amount_token = amount_token;
    launch_record.amount_quote = amount_quote;
    Ok(())
}
