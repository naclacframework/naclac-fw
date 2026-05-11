# `naclac-macros`

`naclac-macros` is the powerful procedural macro engine that powers the Naclac framework. It is responsible for dynamically generating high-performance, boilerplate-free instruction dispatchers, account validation logic, and Cross-Program Invocation (CPI) wrappers.

## ⚙️ Core Architecture

This crate heavily leverages the `syn` and `quote` crates to parse developer-defined structs and functions, transforming them into optimized Solana Virtual Machine (SVM) instructions.

### 1. `#[naclac_program]` (Instruction Dispatcher)
Defined in `program.rs`, this macro serves as the entry point for the smart contract.
- It parses all public functions inside the marked module, treating them as instructions.
- It generates a highly optimized `match` block based on a computed 8-byte discriminator (SHA256 hash of `global:<function_name>`).
- It automatically rewrites function signatures, injecting the necessary `Context` and lifetime constraints (`'info`) to ensure zero-copy pointer safety throughout the instruction's execution.

### 2. `#[derive(Accounts)]` (Account Hydration & Validation)
Defined in `accounts.rs` and the `instruction/` submodule, this macro processes the structs that define the accounts required by an instruction.
- **Immediate Metadata Checks**: Validates raw `AccountInfo` properties (`is_signer`, `is_writable`, expected owners, address matching).
- **Relational Checks**: Enforces cross-account relationships (e.g., `has_one`) after all accounts are successfully loaded.
- **Hydration Pipeline**: Safely transforms raw byte slices into strongly-typed `KeyedRef`/`KeyedRefMut` zero-copy pointer guards.
- **Teardown Pipeline**: Automatically persists mutated zero-copy data and bumps back to the SVM, handles reallocation, and performs safe account closures.

### 3. `#[component]` (Data Representation)
Defined in `component.rs`, this macro prepares Rust structs for on-chain storage.
- Automatically calculates and injects an 8-byte discriminator.
- In Zero-Copy mode, it forcefully applies `#[repr(C, packed)]` to ensure predictable memory layouts and derives `Pod` and `Zeroable`.

### 4. `#[event]` (Logging)
Defined in `event.rs`, this macro converts structs into on-chain event emitters.
- Computes discriminants at compile-time.
- Automatically calculates byte sizes and injects structural padding to satisfy `bytemuck` alignment requirements in zero-copy modes.

## 🚀 Execution Modes (Macro Output)

The procedural macros are context-aware and will dramatically alter their generated Rust code based on the active execution target:

1. **Solana Program + Borsh**: The macros emit standard `BorshSerialize` and `BorshDeserialize` implementations. Instruction arguments are heap-allocated via `Vec` and `String`.
2. **Solana Program + Zero-Copy**: The macros emit highly optimized `naclac_from_bytes` slicing logic. They automatically rewrite `ZcVec` to `Span<'info, T>` and `ZcString` to `ZcString<'info>`, bypassing the allocator entirely for instruction parsing.
3. **Pinocchio + Zero-Copy**: The macros emit code gated by `#[cfg(feature = "pinocchio")]`. They aggressively utilize raw pointers and Pinocchio-native structs (`AccountView`, `pinocchio_system`), completely avoiding the heavy `solana-program` crate for absolute minimal compute overhead.

## 🔒 Security Engine

The `instruction` module (`parser.rs` and `security.rs`) forms the backbone of Naclac's security model. It parses `#[account(...)]` attributes to dynamically generate `if` statements that validate PDA seeds (`seeds`, `bump`), ownership (`owner`, `token::program`), and initialization state (`init`, `init_if_needed`). All of this logic is injected *before* the developer's core instruction logic is executed, ensuring a completely secure environment.
