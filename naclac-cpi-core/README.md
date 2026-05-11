# Naclac CPI Core

`naclac-cpi-core` is the engine responsible for parsing program interfaces and generating the Rust code required for Cross-Program Invocations (CPI) within the Naclac framework.

It acts as the shared logic layer for both the internal `#[naclac_program]` macro (used by Naclac smart contracts) and the external `#[naclac_interface]` macro (used by callers in the "outside world").

## Key Features

- **Multi-Backend Support**: Generates code that can dynamically switch between standard Solana (`solana_program`) and Pinocchio (`no_std`) execution paths.
- **Hybrid Serialization**: Automatically chooses between **Borsh** (for dynamic types like `Vec` and `String`) and **Zero-Copy** (via `bytemuck` for POD types) to minimize compute unit usage.
- **Safety-First Code Generation**: Utilizes `MaybeUninit` and raw pointer abstractions to handle stack-allocated instruction buffers safely across backends.

## Architecture

The crate is structured around three main modules:

1. **`codegen`**: The primary code generation engine. It transforms `CpiInstruction` metadata into high-performance Rust helper functions.
2. **`automatic`**: Logic to extract instruction metadata from a module's items, calculating Anchor-compatible 8-byte discriminators using Sha256.
3. **`universal`**: A unified parser for interfaces that applies global configuration (like `zero_copy`) to extracted instructions.

## Security & Safety

This crate relies on `unsafe` Rust to achieve its performance targets (such as zero-allocation CPI data construction). Every `unsafe` block in the generated code is documented with specific safety invariants, ensuring that memory layout conversions and raw pointer casts are sound within the context of the Solana/SBF runtime.
