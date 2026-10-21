#![no_std]
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

pub mod components;
pub mod constants;
pub mod events;
pub mod instructions;

use instructions::*;

#[program]
pub mod events_prog {
    pub fn init_counter(ctx: Context<InitCounter>) -> Result {
        init_counter::init_counter(ctx)
    }

    pub fn init_counter_unchecked(ctx: Context<InitCounterUnchecked>) -> Result {
        init_counter_unchecked::init_counter_unchecked(ctx)
    }

    pub fn increment_counter(ctx: Context<IncrementCounter>) -> Result {
        increment_counter::increment_counter(ctx)
    }

    pub fn touch_counter_explicit_bump(ctx: Context<TouchCounterExplicitBump>) -> Result {
        touch_counter_explicit_bump::touch_counter_explicit_bump(ctx)
    }

    pub fn log_event(ctx: Context<LogEvent>, data: Vec<u8>) -> Result {
        log_event::log_event(ctx, data)
    }

    pub fn emit_via_self_cpi_baseline(ctx: Context<EmitViaSelfCpiBaseline>) -> Result {
        emit_via_self_cpi_baseline::emit_via_self_cpi_baseline(ctx)
    }

    pub fn no_cpi_baseline(ctx: Context<NoCpiBaseline>) -> Result {
        no_cpi_baseline::no_cpi_baseline(ctx)
    }

    pub fn log_event_signed(ctx: Context<LogEventSigned>, data: [u8; 8]) -> Result {
        log_event_signed::log_event_signed(ctx, data)
    }

    pub fn emit_via_self_cpi_signed_baseline(ctx: Context<EmitViaSelfCpiSignedBaseline>) -> Result {
        emit_via_self_cpi_signed_baseline::emit_via_self_cpi_signed_baseline(ctx)
    }

    pub fn emit_via_sol_log_data_baseline(ctx: Context<EmitViaSolLogDataBaseline>) -> Result {
        emit_via_sol_log_data_baseline::emit_via_sol_log_data_baseline(ctx)
    }

    pub fn emit_via_self_cpi_sized(ctx: Context<EmitViaSelfCpiSized>, size: u32) -> Result {
        emit_via_self_cpi_sized::emit_via_self_cpi_sized(ctx, size)
    }

    pub fn emit_via_sol_log_data_sized(ctx: Context<EmitViaSolLogDataSized>, size: u32) -> Result {
        emit_via_sol_log_data_sized::emit_via_sol_log_data_sized(ctx, size)
    }

    pub fn event_cpi_smoke(ctx: Context<EventCpiSmoke>) -> Result {
        event_cpi_smoke::event_cpi_smoke(ctx)
    }

    pub fn event_cpi_emit(ctx: Context<EventCpiEmit>) -> Result {
        event_cpi_emit::event_cpi_emit(ctx)
    }

    pub fn emit_via_self_cpi_no_program(ctx: Context<EmitViaSelfCpiNoProgram>) -> Result {
        emit_via_self_cpi_no_program::emit_via_self_cpi_no_program(ctx)
    }

    pub fn event_cpi_noop(ctx: Context<EventCpiEmit>) -> Result {
        event_cpi_emit::event_cpi_noop(ctx)
    }

    pub fn event_cpi_log_one(ctx: Context<EventCpiEmit>) -> Result {
        event_cpi_emit::event_cpi_log_one(ctx)
    }

    pub fn event_cpi_emit_one(ctx: Context<EventCpiEmit>) -> Result {
        event_cpi_emit::event_cpi_emit_one(ctx)
    }
}
