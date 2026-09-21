#![no_std]
use naclac_lang::prelude::*;

declare_id!("8MWkme4Dfe5NBPh17itoSro62n4FTGLmPNXYFRCUzu7A");

pub mod components;
pub mod instructions;
pub mod systems;
pub mod events;
pub mod errors;
pub mod constants;

use instructions::*;

#[program]
pub mod donation_relay {
    pub fn donate_pubkey_config_id_with_payer_v1(
        ctx: Context<DonatePubkeyConfigIdWithPayerV1>,
        args: DonatePubkeyConfigIdWithPayerV1Args,
    ) -> Result {
        donate_pubkey_config_id_with_payer_v1::donate_pubkey_config_id_with_payer_v1(ctx, args)
    }
}
