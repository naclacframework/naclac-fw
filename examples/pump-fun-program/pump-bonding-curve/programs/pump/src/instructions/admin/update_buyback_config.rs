use naclac_lang::prelude::*;
use crate::components::Global;
use crate::constants::{BUYBACK_VAULT_SEED, GLOBAL_SEED, PUMP_FEES_PROGRAM_ID};
use crate::errors::PumpError;

// Real accounts (confirmed via 2 real mainnet transactions, decoded directly
// off `getTransaction`, plus `reference/fee-tier-probe/src/bin/probe65.rs`
// run against the real deployed `pump.so`): `global`(mut), `authority`(mut,
// signer, must equal `global.authority`). `buyback_basis_points: Option<u64>`
// -- `Some(x)` sets `Global.buyback_basis_points`; `None` leaves it
// unchanged (confirmed empirically, not the "reset to 0" alternative).
// `remaining_accounts` is separately optional: 0 leaves
// `Global.buyback_fee_recipients` untouched, exactly 8 replaces all of it,
// independent of whether `buyback_basis_points` is `Some` or `None`. Any
// other remaining_accounts count is rejected with the real
// `WrongBuybackFeeRecipientsCount` (6061) error -- real message confirmed
// via probe: "buyback fee recipients require exactly 8 remaining accounts
// (or none)".
//
// Real `pump.so` itself does NOT verify the 8 addresses are genuine
// `pump_fees::BuybackVault` PDAs at this instruction -- confirmed via
// `probe66.rs`, which patched a real, currently-deployed `Global` to an
// attacker-controlled, never-before-seen address and had real `pump.so`
// pay real fee lamports into it at trade time with no rejection. This
// reimplementation deliberately diverges: `buyback_vault_bumps` lets it
// verify each address against the real PDA derivation (`[BUYBACK_VAULT_SEED,
// &[index]]` under `pump_fees`) before accepting it, so a compromised or
// malicious `global.authority` can't redirect buyback fees to an arbitrary
// wallet the way the real protocol allows.
#[derive(Accounts)]
pub struct UpdateBuybackConfig {
    #[account(mut, seeds = [GLOBAL_SEED], bump)]
    pub global: Account<Global>,

    #[account(mut)]
    pub authority: Signer,
}

pub fn update_buyback_config(
    ctx: Context<UpdateBuybackConfig>,
    buyback_basis_points: Option<u64>,
    buyback_vault_bumps: [u8; 8],
) -> Result {
    require!(
        ctx.accounts.authority.address() == ctx.accounts.global.authority,
        PumpError::NotAuthorized
    );

    if let Some(bps) = buyback_basis_points {
        ctx.accounts.global.buyback_basis_points = bps;
    }

    match ctx.remaining_accounts.len() {
        0 => {}
        8 => {
            let mut recipients = [Address::default(); 8];
            for (i, (slot, account)) in recipients.iter_mut().zip(ctx.remaining_accounts.iter()).enumerate() {
                let expected =
                    derive_program_address(&[BUYBACK_VAULT_SEED, &[i as u8]], buyback_vault_bumps[i], &PUMP_FEES_PROGRAM_ID);
                require!(account.address() == expected, PumpError::InvalidBuybackFeeRecipient);
                *slot = account.address();
            }
            ctx.accounts.global.buyback_fee_recipients = recipients;
        }
        _ => return Err(PumpError::WrongBuybackFeeRecipientsCount.into()),
    }

    msg!("Buyback config successfully updated");
    Ok(())
}
