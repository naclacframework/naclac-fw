# Naclac Interface

`naclac-interface` provides a clean, type-safe way for external programs—such as those built with **Anchor**, raw **solana-program**, or raw **Pinocchio**—to call into a Naclac-based smart contract via Cross-Program Invocations (CPI).

It is designed to be the only dependency needed for the "outside world" to interact with the Naclac ecosystem.

## Usage

Simply add `naclac-interface` to your `Cargo.toml` and define the interface of the program you wish to call:

```rust
use naclac_interface::naclac_interface;

#[naclac_interface]
mod my_program_interface {
    use super::*;

    pub fn set_data(ctx: Context<SetData>, value: u64) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct SetData<'info> {
    pub authority: Signer<'info>,
    pub my_account: Account<'info, MyAccount>,
}
```

## How It Works

The `#[naclac_interface]` macro generates a `cpi` module containing helper functions that match the program's instructions.

### Automatic Routing
The generated code is **backend-agnostic**. It uses `naclac-lang`'s unified runtime to automatically detect your environment:
- If you are building a standard Solana program, it will use standard syscalls and Borsh/Zero-Copy serialization as appropriate.
- If you are building a **Pinocchio** (`no_std`) program, it will automatically switch to stack-allocated buffers and zero-copy pointer mapping.

### Performance
By using the interface, you benefit from Naclac's optimized memory management. If the target program is configured with `#[naclac_interface(zero_copy)]`, the generated helpers will use `bytemuck` to encode instruction data, significantly reducing the compute units required for the call.

## Dependency Note
While this crate uses `naclac-lang` and `naclac-macros` internally, you do **not** need to add them to your `Cargo.toml`. `naclac-interface` acts as a facade, re-exporting everything necessary for a seamless developer experience.
