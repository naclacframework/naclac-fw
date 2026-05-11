# Naclac Lang

`naclac-lang` is the core runtime abstraction library powering the Naclac Solana framework. It acts as the structural glue between the developer's high-level logic and the underlying Solana Virtual Machine (SVM).

This crate provides the core primitives, traits, and zero-cost abstractions that the `naclac-macros` engine relies upon to generate secure, optimized smart contracts.

## Architecture & Execution Modes

The fundamental goal of `naclac-lang` is to allow developers to write Solana programs *once* and seamlessly compile them across three distinct execution environments without changing their core business logic:

1. **Solana Program + Borsh**: 
   The standard Solana development experience. Relies on `solana_program`, heap allocations (`Vec`, `String`), and dynamic Borsh serialization. It provides maximum compatibility with existing standard libraries and CPI tools at the cost of higher compute and allocation overhead.
2. **Solana Program + Zero-Copy**: 
   A hybrid approach. Retains the standard `solana_program` context but bypasses Borsh. Instead, it leverages `bytemuck::Pod` and `Zeroable` traits to memory-map account data directly from the instruction buffer, drastically reducing CPU overhead while retaining standard CPI capabilities.
3. **Pinocchio + Zero-Copy**: 
   The maximum performance environment. Compiles under `#![no_std]` utilizing the `pinocchio` crate. All heap allocations are strictly forbidden. Memory is managed exclusively via raw pointer slices and `Span`s, and CPI calls dynamically construct their signer seeds entirely on the stack using `MaybeUninit`. 

## Core Abstractions

- **The Unified Prelude**: Exposes `AccountInfo`, `Pubkey`, `CpiContext`, and standard SPL integrations (`token`, `associated_token`) that dynamically resolve to either `solana_program` or `pinocchio` equivalents based on your `Cargo.toml` features.
- **Account Wrappers**: Zero-cost abstraction types (`Signer`, `Program`, `Interface`, `AccountLoader`) that wrap raw SVM pointers, ensuring strict safety invariants (such as execution privileges or ownership) are validated upon context initialization.
- **Span & ZcString**: Strictly non-allocating vector and string equivalents (`ZcVec`, `ZcString`) for `no_std` environments, generated directly from raw instruction byte streams.

## Safety Invariants

This crate extensively uses `unsafe` Rust to achieve its zero-copy and zero-allocation guarantees.
- All SVM pointer conversions (e.g., from Pinocchio's `Address` to Naclac's `Pubkey`) rely on identical transparent memory layouts `#[repr(transparent)]`.
- All `AccountView` operations securely bind to runtime-guaranteed pointers that live for the entirety of the program invocation.
- All stack-allocated CPI seed bindings (`MaybeUninit`) are bounded to the exact lifetimes of the Cross-Program Invocation execution.

## Usage

You rarely need to interact with `naclac-lang` directly. It is automatically imported and utilized by the `naclac::prelude::*` glob import when using the main framework.
