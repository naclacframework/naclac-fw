use naclac_lang::prelude::*;
use crate::components::BuybackVault;
use crate::constants::BUYBACK_VAULT_SEED;
use crate::errors::FeesError;
use crate::events::SweepBuybackEvent;

// naclac bans on-chain `find_program_address`, so `buyback_vault`'s seeds
// (dynamic on `index`) need an explicit, client-computed bump — `BuybackVault`
// has no stored `bump` field (fees-02), so this can't be read back either.
#[derive(Accounts)]
#[instruction(index: u8, buyback_vault_bump: u8)]
pub struct SweepBuyback {
    /// SAFETY: only used as a lamport/token-transfer destination; not
    /// deserialized. `destination_ata` is separately typed/validated as a
    /// real `TokenAccount`.
    #[account(mut)]
    pub destination: AccountInfo,
    #[account(mut)]
    pub authority: Signer,
    #[account(mut, seeds = [BUYBACK_VAULT_SEED, &[index]], bump = buyback_vault_bump)]
    pub buyback_vault: Account<BuybackVault>,
    #[account(mut)]
    pub buyback_vault_ata: Account<TokenAccount>,
    #[account(mut)]
    pub destination_ata: Account<TokenAccount>,
    pub system_program: Program<System>,
    pub associated_token_program: Program<AssociatedToken>,
    pub mint: Account<Mint>,
    pub token_program: Program<Token>,
}

pub fn sweep_buyback(ctx: Context<SweepBuyback>, index: u8, buyback_vault_bump: u8) -> Result {
    require!(
        ctx.accounts.authority.address() == ctx.accounts.buyback_vault.authority,
        FeesError::NotAuthorized
    );

    // fees-06#9: BuybackVault's rate limit is `>` (strict) + hard revert —
    // the opposite of SocialFeePda's `>=` + soft no-op. A negative
    // claim_rate_limit means "unlimited" (check bypassed entirely).
    let now = unix_timestamp()?;
    if ctx.accounts.buyback_vault.claim_rate_limit >= 0 {
        let elapsed = now.saturating_sub(ctx.accounts.buyback_vault.last_claimed);
        require!(
            elapsed > ctx.accounts.buyback_vault.claim_rate_limit,
            FeesError::ClaimRateLimitExceeded
        );
    }

    let signer_seeds: &[&[u8]] = &[&BUYBACK_VAULT_SEED, &[index], &[buyback_vault_bump]];
    let signer_seeds: &[&[&[u8]]] = &[signer_seeds];

    let token_amount = ctx.accounts.buyback_vault_ata.amount();
    let decimals = ctx.accounts.mint.decimals();
    ctx.accounts.token_program.transfer_checked_signed(
        TransferCheckedAccounts {
            from: &mut ctx.accounts.buyback_vault_ata,
            mint: &ctx.accounts.mint,
            to: &mut ctx.accounts.destination_ata,
            authority: &ctx.accounts.buyback_vault,
        },
        token_amount,
        decimals,
        signer_seeds,
    )?;

    // fees-05 #6: leave `buyback_vault` rent-exempt, never drain the full balance.
    let vault_data_len = 8 + core::mem::size_of::<BuybackVault>();
    let rent_exempt_minimum = Rent::get()?.try_minimum_balance(vault_data_len)?;
    let vault_lamports = ctx.accounts.buyback_vault.to_account_info().lamports();
    let sol_amount = vault_lamports.saturating_sub(rent_exempt_minimum);
    if sol_amount > 0 {
        ctx.accounts.buyback_vault.sub_lamports(sol_amount)?;
        ctx.accounts.destination.add_lamports(sol_amount)?;
    }
    ctx.accounts.buyback_vault.last_claimed = now;

    emit!(SweepBuybackEvent {
        sol_amount,
        token_amount,
        destination: ctx.accounts.destination.address(),
        buyback_vault: ctx.accounts.buyback_vault.address(),
        mint: ctx.accounts.mint.address(),
        index,
    });

    Ok(())
}
