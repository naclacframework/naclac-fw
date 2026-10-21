//! Instructions that create the account and write its enum fields.

use crate::components::State;
use crate::types::{Action, Color, ModeArgs, StateChanged};
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct InitState {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        seeds = [b"state"],
        bump
    )]
    pub state: Account<State>,
    pub system_program: Program<System>,
}

pub fn init_state(ctx: Context<InitState>) -> Result {
    let state = &mut ctx.accounts.state;
    state.admin = ctx.accounts.payer.address();
    state.color = Color::Green;
    state.action = Action::SetFee { fee_bps: 25 };
    Ok(())
}

#[derive(Accounts)]
pub struct Mutate {
    pub signer: Signer,
    #[account(mut, seeds = [b"state"], bump)]
    pub state: Account<State>,
}

pub fn set_action(ctx: Context<Mutate>, action: Action) -> Result {
    ctx.accounts.state.action = action;
    Ok(())
}

pub fn set_color(ctx: Context<Mutate>, color: Color) -> Result {
    ctx.accounts.state.color = color;
    Ok(())
}

pub fn bench_noop(_ctx: Context<Mutate>) -> Result {
    Ok(())
}

pub fn bench_u8_set_const(ctx: Context<Mutate>) -> Result {
    ctx.accounts.state.counter = 2;
    Ok(())
}

pub fn bench_color_set_const(ctx: Context<Mutate>) -> Result {
    ctx.accounts.state.color = Color::Blue;
    Ok(())
}

pub fn bench_color_get(ctx: Context<Mutate>) -> Result {
    ctx.accounts.state.counter = match ctx.accounts.state.color {
        Color::Red => 0,
        Color::Green => 1,
        Color::Blue => 2,
    };
    Ok(())
}

pub fn bench_action_set_const(ctx: Context<Mutate>) -> Result {
    let to = ctx.accounts.signer.address();
    ctx.accounts.state.action = Action::Transfer { amount: 5, to };
    Ok(())
}

pub fn bench_action_get(ctx: Context<Mutate>) -> Result {
    ctx.accounts.state.counter = match ctx.accounts.state.action {
        Action::Pause => 0,
        Action::SetFee { .. } => 1,
        Action::Transfer { .. } => 2,
        Action::Paint(_) => 3,
    };
    Ok(())
}

pub fn set_mode_with_args(ctx: Context<Mutate>, args: ModeArgs) -> Result {
    ctx.accounts.state.color = args.mode;
    ctx.accounts.state.counter = args.amount as u8;
    Ok(())
}

pub fn emit_state_changed(ctx: Context<Mutate>) -> Result {
    let state = &ctx.accounts.state;
    emit!(StateChanged {
        action: state.action,
        color: state.color,
        counter: state.counter,
    });
    Ok(())
}
