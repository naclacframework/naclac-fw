//! Instructions that create the account and write its enum fields.

use crate::state::State;
use crate::types::{
    Action, Color, DynArgs, Level, ListArgs, ModeArgs, MixedEvent, Signed, StateChanged,
    StateLog, Wide,
};
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
    state.color.set(Color::Green);
    state.action.set(Action::SetFee { fee_bps: 25 });
    state.level.set(Level::Mid);
    state.signed.set(Signed::Neg);
    state.wide.set(Wide::Small(5));
    Ok(())
}

#[derive(Accounts)]
pub struct Mutate {
    pub signer: Signer,
    #[account(mut, seeds = [b"state"], bump)]
    pub state: Account<State>,
}

pub fn set_action(ctx: Context<Mutate>, action: Action) -> Result {
    ctx.accounts.state.action.set(action);
    Ok(())
}

pub fn set_color(ctx: Context<Mutate>, color: Color) -> Result {
    ctx.accounts.state.color.set(color);
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
    ctx.accounts.state.color.set(Color::Blue);
    Ok(())
}

pub fn bench_color_get(ctx: Context<Mutate>) -> Result {
    let color = ctx.accounts.state.color.get()?;
    ctx.accounts.state.counter = match color {
        Color::Red => 0,
        Color::Green => 1,
        Color::Blue => 2,
    };
    Ok(())
}

pub fn bench_action_set_const(ctx: Context<Mutate>) -> Result {
    let to = ctx.accounts.signer.address();
    ctx.accounts
        .state
        .action
        .set(Action::Transfer { amount: 5, to });
    Ok(())
}

pub fn bench_action_get(ctx: Context<Mutate>) -> Result {
    let action = ctx.accounts.state.action.get()?;
    ctx.accounts.state.counter = match action {
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
        color: state.color,
        action: state.action,
        counter: state.counter,
    });
    Ok(())
}

pub fn emit_state_log(ctx: Context<Mutate>) -> Result {
    let state = &ctx.accounts.state;
    let colors = Vec::from([
        ZcEnum::<Color>::new(Color::Red),
        ZcEnum::<Color>::new(Color::Blue),
    ]);
    emit!(StateLog {
        head: 7,
        color: state.color,
        colors,
        maybe: Some(state.color),
        action: state.action,
    });
    Ok(())
}

pub fn dyn_args(ctx: Context<Mutate>, args: DynArgs, more: Vec<u32>) -> Result {
    let mut total = args.tag as usize
        + args.values.len()
        + args.note.len()
        + args.heap.len()
        + more.len();
    if args.maybe.is_some() {
        total += 100;
    }
    ctx.accounts.state.counter = total as u8;
    Ok(())
}

pub fn text_lists(
    ctx: Context<Mutate>,
    args: ListArgs,
    names: Vec<String>,
    blobs: Vec<Vec<u8>>,
) -> Result {
    let total = args.words.iter().map(|w| w.len()).sum::<usize>()
        + names.iter().map(|n| n.len()).sum::<usize>()
        + blobs.iter().map(|b| b.len()).sum::<usize>();
    ctx.accounts.state.counter = total as u8;
    Ok(())
}

pub fn enum_lists(
    ctx: Context<Mutate>,
    heap: Vec<Color>,
    view: ZcVec<ZcEnum<Color>>,
) -> Result {
    let heap_blue = heap.iter().filter(|c| matches!(c, Color::Blue)).count();
    let mut view_blue = 0;
    for slot in view.iter() {
        if matches!(slot.get()?, Color::Blue) {
            view_blue += 1;
        }
    }
    ctx.accounts.state.counter = (heap_blue * 10 + view_blue) as u8;
    Ok(())
}

pub fn emit_mixed_event(ctx: Context<Mutate>) -> Result {
    emit!(MixedEvent {
        tag: 9u8,
        amount: 123_456_789u64,
        color: ctx.accounts.state.color,
    });
    Ok(())
}

pub fn measure_cu(ctx: Context<Mutate>) -> Result {
    let to = ctx.accounts.signer.address();
    let state = &mut ctx.accounts.state;

    sol_log_compute_units();
    sol_log_compute_units();

    sol_log_compute_units();
    state.counter = 2;
    sol_log_compute_units();

    // Reads come before writes so the compiler cannot reuse a value this
    // instruction just stored instead of validating the account's bytes.
    sol_log_compute_units();
    state.counter = match state.color.get()? {
        Color::Red => 0,
        Color::Green => 1,
        Color::Blue => 2,
    };
    sol_log_compute_units();

    sol_log_compute_units();
    state.counter = match state.action.get()? {
        Action::Pause => 0,
        Action::SetFee { .. } => 1,
        Action::Transfer { .. } => 2,
        Action::Paint(_) => 3,
    };
    sol_log_compute_units();

    sol_log_compute_units();
    state.color.set(Color::Blue);
    sol_log_compute_units();

    sol_log_compute_units();
    state.action.set(Action::Transfer { amount: 5, to });
    sol_log_compute_units();
    Ok(())
}
