#![no_std]
use naclac_lang::prelude::*;

declare_id!("2bfcZfVHyYj2ifpRj3UXtutcV15FXYrgZUaY7x52pDwv");

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod types;

use {instructions::*, types::Status};

#[program]
pub mod treasury {
    #[discriminator = 1]
    pub fn create_treasury(ctx: Context<CreateTreasury>, threshold: u8) -> Result {
        create_treasury::create_treasury(ctx, threshold)
    }

    #[discriminator = 2]
    pub fn add_member(ctx: Context<AddMember>, member: Address) -> Result {
        add_member::add_member(ctx, member)
    }

    #[discriminator = 3]
    pub fn remove_member(ctx: Context<RemoveMember>, member: Address) -> Result {
        remove_member::remove_member(ctx, member)
    }

    #[discriminator = 4]
    pub fn set_status(ctx: Context<SetStatus>, status: Status) -> Result {
        set_status::set_status(ctx, status)
    }

    #[discriminator = 5]
    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result {
        deposit::deposit(ctx, amount)
    }

    #[discriminator = 6]
    pub fn propose(
        ctx: Context<Propose>,
        id: u64,
        recipient: Address,
        amount: u64,
        deadline: i64,
    ) -> Result {
        propose::propose(ctx, id, recipient, amount, deadline)
    }

    #[discriminator = 7]
    pub fn approve(ctx: Context<Approve>) -> Result {
        approve::approve(ctx)
    }

    #[discriminator = 8]
    pub fn execute(ctx: Context<Execute>) -> Result<u64> {
        execute::execute(ctx)
    }

    #[discriminator = 9]
    pub fn close_proposal(ctx: Context<CloseProposal>) -> Result {
        close_proposal::close_proposal(ctx)
    }

    #[discriminator = 10]
    pub fn init_note(ctx: Context<InitNote>, id: u64) -> Result {
        init_note::init_note(ctx, id)
    }

    #[discriminator = 11]
    pub fn resize_note(ctx: Context<ResizeNote>, new_space: u64) -> Result {
        resize_note::resize_note(ctx, new_space)
    }

    #[discriminator = 12]
    pub fn merge_notes(ctx: Context<MergeNotes>, amount: u64) -> Result {
        merge_notes::merge_notes(ctx, amount)
    }
}
