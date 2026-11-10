//! # Naclac Derive
//!
//! The procedural macros of the Naclac framework. This file holds only the
//! macro entry points; each forwards to the module that implements it.
//!
//! ## Layouts and runtimes
//! A program picks a runtime (`solana` or `pinocchio`) and one account-data layout
//! (`bytemuck`, `wincode` or `borsh`), independently of each other:
//! - **`bytemuck`** (zero-copy): native, zero-copy instruction parsing and account access, on either runtime.
//! - **`wincode`**: heap-allocated accounts and arguments serialized with Wincode, on either runtime.
//! - **`borsh`**: heap-allocated accounts and arguments serialized with Borsh, on the `solana` runtime only.

extern crate proc_macro;

mod account;
mod accounts;
mod common;
mod constant;
mod constraint_args;
mod defined_type;
mod emit;
mod error_code;
mod event;
mod event_cpi;
mod instruction_args;
mod pod_struct_checks;
mod program;
mod serialized_bounds;
mod system;
mod type_classify;
mod zc_enum;

use proc_macro::TokenStream;

/// Defines an on-chain account data structure.
///
/// In the `bytemuck` layout (on either runtime), generates a `#[repr(C)]`
/// struct with `Pod`/`Zeroable`/`NaclacPod` impls for raw byte-cast access. In
/// the `borsh` layout, generates standard `BorshSerialize`/`BorshDeserialize` impls instead.
///
/// Takes an optional `discriminator = <int | [ints]>` argument (1 to 8 bytes)
/// replacing the default name-derived 8-byte discriminator.
#[proc_macro_attribute]
pub fn account(attr: TokenStream, item: TokenStream) -> TokenStream {
    account::expand(attr, item)
}

/// Compile-time math-safety engine for instruction logic functions: bans
/// floats, checked-math-rewrites arithmetic operators, catches literal
/// overflow/div-by-zero at compile time, and (via attribute args) supports
/// rounding control, decimal-scale tracking, proptest fuzz-test generation,
/// and Kani proof-harness generation. See `system.rs`'s module docs for the
/// full attribute syntax.
#[proc_macro_attribute]
pub fn system(attr: TokenStream, item: TokenStream) -> TokenStream {
    system::expand(attr, item)
}

/// The main entrypoint macro for a Naclac smart contract.
///
/// This generates the overarching instruction dispatcher and handles Cross-Program
/// Invocation (CPI) module generation. The layout is detected from the crate's own
/// feature flags, not from an argument here.
///
/// An instruction's IDL docs are the doc comments written on its handler
/// inside this module; docs on a function the handler calls are not read.
///
/// A `ZcVec` or `ZcString` parameter may carry a limit in its type:
/// `ZcVec<u64, 8>` holds at most 8 elements and `ZcString<32>` at most 32
/// bytes; plain `ZcVec<u64>` and `ZcString` have none. Longer data fails with
/// `InvalidInstructionData` before the handler runs. The same works on the
/// fields of an `#[instruction_args]` struct. Write the same type wherever
/// the value is passed on.
#[proc_macro_attribute]
pub fn program(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Some(err) = common::diagnostics::reject_nonempty_attr(&attr, "program") {
        return err;
    }
    program::expand(attr, item)
}

/// Generates standardized custom error codes for the program: each variant's
/// code is `offset + discriminant`, where `offset` is 6000 unless set with
/// `#[error_code(offset = N)]`. Every code in the crate must be unique
/// (`#[program]` rejects overlaps).
#[proc_macro_attribute]
pub fn error_code(attr: TokenStream, item: TokenStream) -> TokenStream {
    error_code::expand(attr, item)
}

/// Defines an event struct.
///
/// In the `bytemuck` layout, generates allocator-free event emission logic
/// directly into the Solana runtime via `sol_invoke`.
#[proc_macro_attribute]
pub fn event(attr: TokenStream, item: TokenStream) -> TokenStream {
    event::expand(attr, item)
}

/// Auto-injects the `event_authority`/`program` accounts a self-CPI event
/// emission (`emit_cpi!()`) needs. Must be applied *above*
/// `#[derive(Accounts)]`, not below it — as a real attribute macro it
/// rewrites the struct's own field list before the derive macro ever sees
/// it, which a derive macro alone cannot do.
#[proc_macro_attribute]
pub fn event_cpi(attr: TokenStream, item: TokenStream) -> TokenStream {
    event_cpi::expand(attr, item)
}

/// Logs an event with `sol_log_data`: `emit!(MyEvent { a, b: 2 });` or
/// `emit!(event_value);`. A struct literal starts from `Default` and assigns
/// each written field through `.into()`. Evaluates to the event.
#[proc_macro]
pub fn emit(input: TokenStream) -> TokenStream {
    emit::expand_emit(input)
}

/// Emits an event as a signed self-CPI: `emit_cpi!(MyEvent { a, b: 2 });`.
/// Takes exactly the event syntax `emit!` takes and evaluates to the event.
/// The handler's context is the variable named `ctx`; a function holding its
/// context under another name calls
/// `naclac_lang::event::emit_cpi(&context, &event)` directly. The handler's
/// accounts struct must carry `#[event_cpi]`.
#[proc_macro]
pub fn emit_cpi(input: TokenStream) -> TokenStream {
    emit::expand_emit_cpi(input)
}

/// Marks a `const` for the IDL; the item itself is never modified. Under
/// `idl-build` it also registers the constant for the IDL.
#[proc_macro_attribute]
pub fn constant(attr: TokenStream, item: TokenStream) -> TokenStream {
    constant::expand(attr, item)
}

/// Adds a setter per key to a constraint plugin's `Args` struct, which
/// `#[derive(Accounts)]` uses to build it from `#[account(namespace::key = ..)]`.
/// Every field must be `Option<..>`.
#[proc_macro_attribute]
pub fn constraint_args(attr: TokenStream, item: TokenStream) -> TokenStream {
    constraint_args::expand(attr, item)
}

/// Derives the necessary validation, deserialization, and constraints for an Accounts struct.
///
/// Generates zero-copy fixed array CPI properties (`[AccountMeta; N]`) regardless of the layout.
#[proc_macro_derive(Accounts, attributes(account, instruction, __naclac_event_cpi))]
pub fn derive_accounts(item: TokenStream) -> TokenStream {
    accounts::expand_derive_accounts(item)
}

/// Derives the serialization for a standalone, non-account, non-event type (a
/// struct or enum used as an instruction argument or a nested field),
/// following the same per-layout routing as `#[account]` and `#[event]`: the
/// `bytemuck` layout gets `Pod`/`Zeroable` (structs) or `ZcCodec` (enums),
/// and the `borsh` and `wincode` layouts get their real derives.
///
/// Enums need a separate zero-copy path because only as many tag values are
/// valid as there are variants, so reinterpreting arbitrary bytes as the enum
/// would be undefined behavior. An enum is stored as its tag followed by the
/// active variant's fields, packed, and decoded through a generated `ZcCodec`
/// that validates the tag and every nested value. `NaclacPod::naclac_try_from_bytes`
/// returns `None` for an invalid one; `naclac_from_bytes` panics.
#[proc_macro_attribute]
pub fn defined_type(attr: TokenStream, item: TokenStream) -> TokenStream {
    defined_type::expand(attr, item)
}

/// Groups instruction arguments into one struct.
///
/// In the `bytemuck` layout a fixed-size struct becomes a `repr(C, packed)`
/// `Pod` with `Clone`, `Copy`, `Zeroable`, `Default` and `NaclacPod`, and a
/// struct with a `String`/`Vec`/`Option`/`ZcVec`/`ZcString` field is read field
/// by field through `NaclacArgs`. In the `borsh` and `wincode` layouts the
/// struct gets `Clone` plus that layout's derives.
#[proc_macro_attribute]
pub fn instruction_args(attr: TokenStream, item: TokenStream) -> TokenStream {
    instruction_args::expand(attr, item)
}
