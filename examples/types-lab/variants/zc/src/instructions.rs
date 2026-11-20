//! Accounts and handlers of the zero-copy family.

use {
    crate::{
        errors::LabError,
        events::{EnumsChanged, LabLog, RosterChanged, ScalarsSet, TextLog},
        state::{Book, Lab, Note},
        types::{
            Action, Color, Entry, Level, NestedArgs, OptionArgs, ScalarArgs, Signed, Wide,
        },
    },
    naclac_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitLab {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        seeds = [b"lab"],
        bump
    )]
    pub lab: Account<Lab>,
    pub system_program: Program<System>,
}

pub fn init_lab(ctx: Context<InitLab>) -> Result {
    let lab = &mut ctx.accounts.lab;
    lab.bump = ctx.bumps.lab;
    lab.authority = ctx.accounts.payer.address();
    lab.color.set(Color::Green);
    lab.action.set(Action::SetFee { fee_bps: 25 });
    lab.level.set(Level::Mid);
    lab.signed.set(Signed::Neg);
    lab.wide.set(Wide::Small(5));
    lab.history[0].set(Color::Blue);
    Ok(())
}

#[derive(Accounts)]
pub struct Mutate {
    pub signer: Signer,
    #[account(mut, seeds = [b"lab"], bump)]
    pub lab: Account<Lab>,
}

pub fn set_scalars(ctx: Context<Mutate>, args: ScalarArgs) -> Result {
    let lab = &mut ctx.accounts.lab;
    lab.flag = args.flag;
    lab.small = args.small;
    lab.short = args.short;
    lab.medium = args.medium;
    lab.large = args.large;
    lab.huge = args.huge;
    lab.tiny_signed = args.tiny_signed;
    lab.short_signed = args.short_signed;
    lab.medium_signed = args.medium_signed;
    lab.large_signed = args.large_signed;
    lab.huge_signed = args.huge_signed;
    lab.single = args.single;
    lab.double = args.double;
    lab.key = args.key;
    emit!(ScalarsSet {
        flag: args.flag,
        small: args.small,
        short: args.short,
        medium: args.medium,
        large: args.large,
        huge: args.huge,
        tiny_signed: args.tiny_signed,
        short_signed: args.short_signed,
        medium_signed: args.medium_signed,
        large_signed: args.large_signed,
        huge_signed: args.huge_signed,
        single: args.single,
        double: args.double,
        key: args.key,
    });
    Ok(())
}

pub fn set_color(ctx: Context<Mutate>, color: Color) -> Result {
    ctx.accounts.lab.color.set(color);
    Ok(())
}

pub fn set_action(ctx: Context<Mutate>, action: Action) -> Result {
    ctx.accounts.lab.action.set(action);
    Ok(())
}

pub fn set_level(ctx: Context<Mutate>, level: Level) -> Result {
    ctx.accounts.lab.level.set(level);
    Ok(())
}

pub fn set_signed(ctx: Context<Mutate>, signed: Signed) -> Result {
    ctx.accounts.lab.signed.set(signed);
    Ok(())
}

pub fn set_wide(ctx: Context<Mutate>, wide: Wide) -> Result {
    ctx.accounts.lab.wide.set(wide);
    Ok(())
}

pub fn set_nested(ctx: Context<Mutate>, args: NestedArgs) -> Result {
    let lab = &mut ctx.accounts.lab;
    lab.limits = args.limits;
    lab.matrix = args.matrix;
    lab.bytes = args.bytes;
    Ok(())
}

pub fn set_maybe_u64(ctx: Context<Mutate>, args: OptionArgs) -> Result {
    ctx.accounts.lab.maybe_u64.set(args.maybe);
    Ok(())
}

pub fn set_maybe_color(ctx: Context<Mutate>, color: Color) -> Result {
    ctx.accounts
        .lab
        .maybe_color
        .set(Some(ZcEnum::<Color>::new(color)));
    Ok(())
}

pub fn clear_maybe_color(ctx: Context<Mutate>) -> Result {
    ctx.accounts.lab.maybe_color.set(None);
    Ok(())
}

pub fn set_text(ctx: Context<Mutate>, name: ZcString) -> Result {
    ctx.accounts.lab.name.try_set(name.as_str())?;
    Ok(())
}

pub fn set_scores(ctx: Context<Mutate>, values: ZcVec<u64>) -> Result {
    let scores = &mut ctx.accounts.lab.scores;
    scores.clear();
    for value in values.iter() {
        scores.try_push(value)?;
    }
    Ok(())
}

pub fn push_score(ctx: Context<Mutate>, value: u64) -> Result {
    let scores = &mut ctx.accounts.lab.scores;
    require!(scores.len() < 4, LabError::RosterFull);
    scores.try_push(value)?;
    Ok(())
}

pub fn pop_score(ctx: Context<Mutate>) -> Result {
    let popped = ctx.accounts.lab.scores.pop();
    require!(popped.is_some(), NaclacError::InvalidInstructionData);
    Ok(())
}

pub fn add_member(ctx: Context<Mutate>, member: Address) -> Result {
    ctx.accounts.lab.members.try_push(member)?;
    Ok(())
}

pub fn remove_member(ctx: Context<Mutate>, index: u8) -> Result {
    let removed = ctx.accounts.lab.members.remove(index as usize);
    require!(removed.is_some(), NaclacError::InvalidInstructionData);
    Ok(())
}

pub fn push_flag(ctx: Context<Mutate>, value: u8) -> Result {
    ctx.accounts.lab.flags.try_push(Bool::new(value != 0))?;
    Ok(())
}

pub fn clear_roster(ctx: Context<Mutate>) -> Result {
    let lab = &mut ctx.accounts.lab;
    lab.scores.clear();
    lab.name.clear();
    lab.members.clear();
    lab.flags.clear();
    Ok(())
}

pub fn set_roster(
    ctx: Context<Mutate>,
    scores: FixedVec<u64, 4>,
    name: FixedString<16>,
) -> Result {
    let lab = &mut ctx.accounts.lab;
    lab.scores = scores;
    lab.name = name;
    Ok(())
}

pub fn bump_counter(ctx: Context<Mutate>) -> Result {
    let lab = &mut ctx.accounts.lab;
    require!(lab.counter < u8::MAX, LabError::Overflowed);
    lab.counter += 1;
    Ok(())
}

pub fn fail_with(_ctx: Context<Mutate>, code: u8) -> Result {
    require!(code != 1, LabError::Rejected);
    Ok(())
}

pub fn emit_log(ctx: Context<Mutate>) -> Result {
    let lab = &ctx.accounts.lab;
    emit!(EnumsChanged {
        color: lab.color,
        action: lab.action,
        level: lab.level,
        signed: lab.signed,
        wide: lab.wide,
    });
    emit!(RosterChanged {
        scores: lab.scores,
        name: lab.name,
    });
    emit!(LabLog {
        head: lab.counter,
        colors: Vec::from([ZcEnum::<Color>::new(Color::Red), lab.color]),
        maybe: lab.maybe_color.get(),
        note: String::from("lab"),
        values: Vec::from([1u64, 2, 3]),
        action: lab.action,
    });
    emit!(TextLog {
        scores: lab.scores,
        name: lab.name,
        note: String::from("text"),
    });
    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
pub struct EmitCpiLog {
    pub signer: Signer,
    #[account(seeds = [b"lab"], bump)]
    pub lab: Account<Lab>,
}

pub fn emit_cpi_log(ctx: Context<EmitCpiLog>) -> Result {
    let lab = &ctx.accounts.lab;
    emit_cpi!(EnumsChanged {
        color: lab.color,
        action: lab.action,
        level: lab.level,
        signed: lab.signed,
        wide: lab.wide,
    });
    emit_cpi!(LabLog {
        head: lab.counter,
        colors: Vec::from([ZcEnum::<Color>::new(Color::Red), lab.color]),
        maybe: lab.maybe_color.get(),
        note: String::from("cpi"),
        values: Vec::from([4u64, 5]),
        action: lab.action,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct InitNote {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        seeds = [b"note"],
        bump
    )]
    pub note: Account<Note>,
    pub system_program: Program<System>,
}

pub fn init_note(ctx: Context<InitNote>) -> Result {
    ctx.accounts.note.bump = ctx.bumps.note;
    Ok(())
}

#[derive(Accounts)]
pub struct TouchOptional {
    pub signer: Signer,
    #[account(mut)]
    pub note: Option<Account<Note>>,
}

pub fn touch_optional(ctx: Context<TouchOptional>) -> Result {
    if let Some(note) = ctx.accounts.note.as_mut() {
        note.value += 1;
    }
    Ok(())
}

#[derive(Accounts)]
pub struct InitBook {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        seeds = [b"book"],
        bump
    )]
    pub book: Account<Book>,
    pub system_program: Program<System>,
}

pub fn init_book(ctx: Context<InitBook>) -> Result {
    ctx.accounts.book.bump = ctx.bumps.book;
    Ok(())
}

#[derive(Accounts)]
pub struct MutateBook {
    pub signer: Signer,
    #[account(mut, seeds = [b"book"], bump)]
    pub book: Account<Book>,
}

pub fn set_entry(ctx: Context<MutateBook>, entry: Entry) -> Result {
    ctx.accounts.book.first = entry;
    Ok(())
}
