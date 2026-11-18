#![allow(unexpected_cfgs)]
// `#[derive(Seeds)]` with `#[seeds(PREFIX, name: Type, ..)]` gives the seeds of
// a PDA that holds no program data, usable on a field of any type.
//
// `#[seeds(PREFIX, field, ..)]` on an `#[account]` generates `seeds(..)`, whose
// seed set signs through `signer_seeds(&bump)`; with `bump = field` naming the
// account's stored bump it also generates `with_signer_seeds`, which hands the
// PDA's seeds to a signed CPI. The prefix is a constant or a literal; each
// field is an integer, an `Address` or a `[u8; N]` of at most 32 bytes.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

pub const SEED_VAULT: &[u8] = b"vault";

#[account]
#[seeds(SEED_VAULT, owner, index, bump = bump)]
pub struct Vault {
    pub owner: Address,
    pub index: u64,
    pub amount: u64,
    pub bump: u8,
}

#[account]
#[seeds(b"literal", key, bump = stored_bump)]
pub struct Literal {
    pub key: [u8; 16],
    pub stored_bump: u8,
}

pub fn vault_signer_count(vault: &Vault) -> usize {
    vault.with_signer_seeds(|signers: &[CpiSigner]| signers.len())
}

pub fn literal_signer_count(literal: &Literal) -> usize {
    literal.with_signer_seeds(|signers: &[CpiSigner]| signers.len())
}


#[derive(Accounts)]
#[instruction(index: u64)]
pub struct Create {
    #[account(mut)]
    pub payer: Signer,

    pub owner: Signer,

    #[account(
        init,
        payer = payer,
        seeds = Vault::seeds(owner.address(), index),
        bump,
    )]
    pub vault: Account<Vault>,

    pub system_program: Program<System>,
}

pub const SLOT_TAG: [u8; 4] = *b"main";

#[account]
#[seeds(b"config", bump = bump)]
pub struct Config {
    pub value: u64,
    pub bump: u8,
}

#[account]
#[seeds(b"slot", tag, version, bump = bump)]
pub struct Slot {
    pub tag: [u8; 4],
    pub version: u32,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct ReadConstantSeeds {
    #[account(seeds = Config::seeds(), bump)]
    pub config: Account<Config>,

    #[account(seeds = Slot::seeds(SLOT_TAG, 2), bump)]
    pub slot: Account<Slot>,
}

pub const CONFIG_PDA: (Address, u8) = Config::__naclac_const_pda(&ID);

pub fn vault_seed_lengths(owner: Address, index: u64) -> (usize, usize) {
    let seeds = Vault::seeds(&owner, &index);
    let bump = [7u8];
    (seeds.as_slices().len(), seeds.as_slices_with_bump(&bump).len())
}

#[account]
#[seeds(b"plain")]
pub struct Plain {
    pub value: u64,
}

#[derive(Accounts)]
pub struct ReadWithoutStoredBump {
    #[account(seeds = Plain::seeds(), bump)]
    pub plain: Account<Plain>,
}

pub fn sign_as_plain(bump: &[u8; 1]) {
    let seeds = Plain::seeds();
    let signer_seeds = seeds.signer_seeds(bump);
    let _signer = signer_seeds.signer();
}

pub const ESCROW_OWNER: Address = naclac_lang::address!("11111111111111111111111111111111");

#[derive(Seeds)]
#[seeds(b"escrow", owner: Address, index: u32)]
pub struct EscrowSeeds;

#[derive(Accounts)]
pub struct ReadSeedsType {
    /// SAFETY: only its address is checked against the seeds; never read.
    #[account(seeds = EscrowSeeds::seeds(ESCROW_OWNER, 3u32))]
    pub escrow: AccountInfo,

    #[account(seeds = EscrowSeeds::seeds(ESCROW_OWNER, 4u32), bump)]
    pub config: Account<Config>,
}

#[account]
#[seeds(b"offer", maker, id: u64)]
pub struct Offer {
    pub maker: Address,
    pub bump: u8,
}

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct TakeOffer {
    #[account(mut)]
    pub maker: Signer,

    #[account(mut, seeds = Offer::seeds(maker.address(), id), bump = offer.bump)]
    pub offer: Account<Offer>,

    /// SAFETY: only its address is checked against the seeds; never read.
    #[account(seeds = EscrowSeeds::seeds(maker.address(), 7u32), bump = 254)]
    pub vault: AccountInfo,
}

pub fn sign_for_offer(accounts: &mut TakeOffer, id: u64, bump: u8) -> usize {
    accounts.with_offer_signer(id, bump, |accounts, signers| {
        let _maker = &mut accounts.maker;
        signers.len()
    })
}

pub fn sign_as_escrow(bump: &[u8; 1]) {
    let index = 5u32;
    let seeds = EscrowSeeds::seeds(&ESCROW_OWNER, &index);
    let signer_seeds = seeds.signer_seeds(bump);
    let _signer = signer_seeds.signer();
}

fn main() {}
