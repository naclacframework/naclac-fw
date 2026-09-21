use naclac_lang::prelude::*;

declare_id!("AUejAUt7m2v3QFitS8f6v7y9FqAnNtFAP4q1s7tgEVmV");

pub mod errors;
pub mod instructions;

use instructions::*;

#[program]
pub mod error_codes {
    pub fn check_amount(ctx: Context<CheckAmount>, amount: u64) -> Result {
        check_amount::check_amount(ctx, amount)
    }

    pub fn check_authority(ctx: Context<CheckAuthority>) -> Result {
        check_authority::check_authority(ctx)
    }

    pub fn require_signer(ctx: Context<RequireSigner>) -> Result {
        require_signer::require_signer(ctx)
    }

    pub fn check_fixed_address(ctx: Context<CheckFixedAddress>) -> Result {
        check_fixed_address::check_fixed_address(ctx)
    }
}
