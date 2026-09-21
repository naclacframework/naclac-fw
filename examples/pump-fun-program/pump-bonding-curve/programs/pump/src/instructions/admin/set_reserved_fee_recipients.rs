use naclac_lang::prelude::*;
use crate::components::Global;
use crate::constants::GLOBAL_SEED;
use crate::errors::PumpError;
use crate::events::ReservedFeeRecipientsEvent;

#[derive(Accounts)]
pub struct SetReservedFeeRecipients {
    #[account(mut, seeds = [GLOBAL_SEED], bump, authority = authority @ PumpError::NotAuthorized)]
    pub global: Account<Global>,

    #[account(mut)]
    pub authority: Signer,
}

// `whitelist_pda` is the only declared arg and stores directly into
// `global.whitelist_pda`. `remaining_accounts[0]` -> `global.reserved_fee_recipient`,
// `remaining_accounts[1..8]` -> `global.reserved_fee_recipients` — undocumented
// in the real IDL, confirmed exactly against real `pump.so`
// (`reference/fee-tier-probe/src/bin/probe68.rs`), mirroring `set_params`'s own
// `fee_recipient`/`fee_recipients` split. Each of the 8 must be rent-exempt,
// matching the real `ConstraintRentExempt` check.
pub fn set_reserved_fee_recipients(ctx: Context<SetReservedFeeRecipients>, whitelist_pda: Address) -> Result {
    require!(ctx.remaining_accounts.len() == 8, PumpError::NotEnoughRemainingAccounts);
    let rent_exempt_minimum = Rent::get()?.try_minimum_balance(0)?;
    let mut recipients = [Address::default(); 8];
    for (i, slot) in recipients.iter_mut().enumerate() {
        let account = &ctx.remaining_accounts[i];
        require!(account.lamports() >= rent_exempt_minimum, PumpError::FeeRecipientNotRentExempt);
        *slot = account.address();
    }

    let reserved_fee_recipients: [Address; 7] = recipients[1..8].try_into().unwrap();

    let global = &mut ctx.accounts.global;
    global.whitelist_pda = whitelist_pda;
    global.reserved_fee_recipient = recipients[0];
    global.reserved_fee_recipients = reserved_fee_recipients;

    let timestamp = unix_timestamp()?;
    emit!(ReservedFeeRecipientsEvent {
        timestamp,
        reserved_fee_recipient: recipients[0],
        reserved_fee_recipients,
    });

    msg!("Reserved fee recipients successfully updated");
    Ok(())
}
