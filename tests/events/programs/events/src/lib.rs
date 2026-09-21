#![no_std]
use naclac_lang::prelude::*;

declare_id!("7LLnnxsXFgpgmx4S8VqjpBrBmpgawekjiSfX1KsCs2AL");

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

    pub fn increment_counter(ctx: Context<IncrementCounter>) -> Result {
        increment_counter::increment_counter(ctx)
    }

    pub fn touch_counter_explicit_bump(ctx: Context<TouchCounterExplicitBump>) -> Result {
        touch_counter_explicit_bump::touch_counter_explicit_bump(ctx)
    }

    pub fn log_event(ctx: Context<LogEvent>, data: ZcVec<u8>) -> Result {
        log_event::log_event(ctx, data)
    }

    pub fn no_cpi_baseline(ctx: Context<NoCpiBaseline>) -> Result {
        no_cpi_baseline::no_cpi_baseline(ctx)
    }

    pub fn emit_via_self_cpi_baseline(ctx: Context<EmitViaSelfCpiBaseline>) -> Result {
        emit_via_self_cpi_baseline::emit_via_self_cpi_baseline(ctx)
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

    pub fn emit_fixed_8(ctx: Context<EmitFixed8>) -> Result {
        emit_fixed_8::emit_fixed_8(ctx)
    }

    pub fn emit_fixed_128(ctx: Context<EmitFixed128>) -> Result {
        emit_fixed_128::emit_fixed_128(ctx)
    }

    pub fn emit_fixed_512(ctx: Context<EmitFixed512>) -> Result {
        emit_fixed_512::emit_fixed_512(ctx)
    }

    pub fn emit_fixed_2048(ctx: Context<EmitFixed2048>) -> Result {
        emit_fixed_2048::emit_fixed_2048(ctx)
    }

    pub fn log_event_fixed_8(ctx: Context<LogEventFixed8>, data: [u8; 8]) -> Result {
        log_event_fixed_8::log_event_fixed_8(ctx, data)
    }

    pub fn log_event_fixed_128(ctx: Context<LogEventFixed128>, data: [u8; 128]) -> Result {
        log_event_fixed_128::log_event_fixed_128(ctx, data)
    }

    pub fn log_event_fixed_512(ctx: Context<LogEventFixed512>, data: [u8; 512]) -> Result {
        log_event_fixed_512::log_event_fixed_512(ctx, data)
    }

    pub fn log_event_fixed_2048(ctx: Context<LogEventFixed2048>, data: [u8; 2048]) -> Result {
        log_event_fixed_2048::log_event_fixed_2048(ctx, data)
    }

    pub fn emit_via_self_cpi_fixed_8(ctx: Context<EmitViaSelfCpiFixed8>) -> Result {
        emit_via_self_cpi_fixed_8::emit_via_self_cpi_fixed_8(ctx)
    }

    pub fn emit_via_self_cpi_fixed_128(ctx: Context<EmitViaSelfCpiFixed128>) -> Result {
        emit_via_self_cpi_fixed_128::emit_via_self_cpi_fixed_128(ctx)
    }

    pub fn emit_via_self_cpi_fixed_512(ctx: Context<EmitViaSelfCpiFixed512>) -> Result {
        emit_via_self_cpi_fixed_512::emit_via_self_cpi_fixed_512(ctx)
    }

    pub fn emit_via_self_cpi_fixed_2048(ctx: Context<EmitViaSelfCpiFixed2048>) -> Result {
        emit_via_self_cpi_fixed_2048::emit_via_self_cpi_fixed_2048(ctx)
    }

    pub fn log_event_signed(ctx: Context<LogEventSigned>, data: [u8; 8]) -> Result {
        log_event_signed::log_event_signed(ctx, data)
    }

    pub fn emit_via_self_cpi_signed_baseline(ctx: Context<EmitViaSelfCpiSignedBaseline>) -> Result {
        emit_via_self_cpi_signed_baseline::emit_via_self_cpi_signed_baseline(ctx)
    }
}
