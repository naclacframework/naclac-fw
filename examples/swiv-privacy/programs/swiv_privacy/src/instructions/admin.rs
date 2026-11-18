//! Protocol initialization, configuration, and authority management.

use {
    crate::{
        constants::MAX_FEE_BPS,
        errors::CustomError,
        events::{ConfigUpdated, ProtocolInitialized},
        state::Protocol,
    },
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitializeProtocol {
    #[account(init, payer = admin, seeds = Protocol::seeds())]
    pub protocol: Account<Protocol>,

    #[account(mut)]
    pub admin: Signer,

    /// SAFETY: Only this wallet's address is persisted; no data or authority is trusted, including when it repeats another account.
    #[account(dup)]
    pub treasury_wallet: AccountInfo,

    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct UpdateConfig {
    #[account(mut)]
    pub admin: Signer,

    #[account(
        mut,
        seeds = Protocol::seeds(),
        bump = protocol.bump,
        constraint = protocol.admin == admin.address() @ CustomError::Unauthorized,
    )]
    pub protocol: Account<Protocol>,

    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct TransferAdmin {
    #[account(mut)]
    pub current_admin: Signer,

    #[account(
        mut,
        seeds = Protocol::seeds(),
        bump = protocol.bump,
        constraint = protocol.admin == current_admin.address() @ CustomError::Unauthorized,
    )]
    pub protocol: Account<Protocol>,
}

#[derive(Accounts)]
pub struct SetPause {
    #[account(
        mut,
        seeds = Protocol::seeds(),
        bump = protocol.bump,
        constraint = protocol.admin == admin.address() @ CustomError::Unauthorized,
    )]
    pub protocol: Account<Protocol>,

    pub admin: Signer,
}

/// Establishes the protocol with a sixty-second settlement wait and an unpaused market.
pub fn initialize_protocol(ctx: Context<InitializeProtocol>, protocol_fee_bps: u64) -> Result {
    require!(protocol_fee_bps <= MAX_FEE_BPS, CustomError::InvalidFee);
    let treasury_wallet = ctx.accounts.treasury_wallet.address();
    require!(
        treasury_wallet != Address::default(),
        CustomError::InvalidTreasury
    );

    let admin = ctx.accounts.admin.address();
    let protocol = &mut ctx.accounts.protocol;
    protocol.admin = admin;
    protocol.treasury_wallet = treasury_wallet;
    protocol.protocol_fee_bps = protocol_fee_bps;
    protocol.paused = Bool::FALSE;
    protocol.batch_settle_wait_duration = 60;
    protocol.total_pools = 0;
    protocol.bump = ctx.bumps.protocol;

    emit!(ProtocolInitialized {
        admin,
        fee_wallet: treasury_wallet,
    });
    Ok(())
}

/// Applies explicitly supplied policy values; malformed optional tags are rejected.
pub fn update_config(
    ctx: Context<UpdateConfig>,
    new_treasury: Opt<Address>,
    new_protocol_fee_bps: Opt<u64>,
    new_batch_settle_wait_duration: Opt<i64>,
) -> Result {
    require!(
        new_treasury.tag_is_valid()
            && new_protocol_fee_bps.tag_is_valid()
            && new_batch_settle_wait_duration.tag_is_valid(),
        CustomError::InvalidOptionalValue
    );

    if let Some(treasury) = new_treasury.get() {
        require!(treasury != Address::default(), CustomError::InvalidTreasury);
    }
    if let Some(fee) = new_protocol_fee_bps.get() {
        require!(fee <= MAX_FEE_BPS, CustomError::InvalidFee);
    }
    if let Some(duration) = new_batch_settle_wait_duration.get() {
        require!(duration >= 0, CustomError::InvalidTime);
    }

    let protocol = &mut ctx.accounts.protocol;
    if let Some(treasury) = new_treasury.get() {
        protocol.treasury_wallet = treasury;
    }
    if let Some(fee) = new_protocol_fee_bps.get() {
        protocol.protocol_fee_bps = fee;
    }
    if let Some(duration) = new_batch_settle_wait_duration.get() {
        protocol.batch_settle_wait_duration = duration;
    }

    emit!(ConfigUpdated {
        treasury: new_treasury,
        protocol_fee_bps: new_protocol_fee_bps,
        batch_settle_wait_duration: new_batch_settle_wait_duration,
    });
    Ok(())
}

/// Transfers protocol authority to a nonzero address authorized by the current admin.
pub fn transfer_admin(ctx: Context<TransferAdmin>, new_admin: Address) -> Result {
    require!(new_admin != Address::default(), CustomError::InvalidAdmin);
    ctx.accounts.protocol.admin = new_admin;
    Ok(())
}

/// Sets the protocol-wide pause flag under the current admin's authority.
pub fn set_pause(ctx: Context<SetPause>, paused: bool) -> Result {
    ctx.accounts.protocol.paused = Bool::new(paused);
    Ok(())
}
