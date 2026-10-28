//! Module-tree fixture: IDL items declared in inline modules, `mod.rs` and flat file modules,
//! `#[path]` modules (top level and inside an inline module), `pub(crate)` nested modules and
//! `cfg`-gated modules and items. `stray.rs` is never declared, so neither IDL may contain it;
//! `disabled.rs` and the `debug-mode`-gated items are compiled out, so neither IDL may contain them.

use naclac_lang::prelude::*;

declare_id!("79qeoneFpg7ES78bAr7hMTDQczeSpMbE1pam5BNkspCg");

pub mod flat;
pub mod nested;

#[path = "elsewhere/moved_here.rs"]
pub mod moved;

#[cfg(feature = "debug-mode")]
pub mod disabled;

#[cfg(not(feature = "debug-mode"))]
pub mod enabled;

pub mod inline {
    use naclac_lang::prelude::*;

    #[constant]
    pub const INLINE_LIMIT: u64 = 7;

    #[event]
    pub struct InlineEvent {
        pub value: u64,
    }

    #[error_code]
    pub enum InlineError {
        /// Inline error.
        Broken,
    }

    #[defined_type]
    pub struct InlinePair {
        pub a: u32,
        pub b: u32,
    }

    #[component]
    pub struct InlineState {
        pub count: u64,
    }

    pub mod deeper {
        use naclac_lang::prelude::*;

        #[derive(Accounts)]
        pub struct DeepAccounts {
            pub signer: Signer,
            #[account(mut)]
            pub state: Account<super::InlineState>,
        }

        #[path = "target.rs"]
        pub mod pathed;
    }
}

#[program]
pub mod idlp_modules_zc {
    pub fn deep(
        ctx: Context<crate::inline::deeper::DeepAccounts>,
        pair: crate::inline::InlinePair,
        mode: crate::flat::child::ChildMode,
    ) -> Result {
        let _ = (ctx, pair, mode);
        Ok(())
    }

    pub fn nested_pda(ctx: Context<crate::nested::inner::NestedAccounts>) -> Result {
        let _ = ctx;
        Ok(())
    }
}
