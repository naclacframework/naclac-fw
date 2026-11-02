#![feature(prelude_import)]
#![no_std]
extern crate core;
#[prelude_import]
use core::prelude::rust_2021::*;
use naclac_lang::prelude::*;
pub const ID: ::naclac_core::prelude::Address = unsafe {
    core::mem::transmute(
        ::solana_address::Address::from_str_const(
            "BUnzdon444sj9eHgaCNgnNk3ckdKc5xSgyax946Uuvau",
        ),
    )
};
pub fn id() -> ::naclac_core::prelude::Address {
    ID
}
const _: () = ::naclac_core::address_literal::assert_program_address(
    "BUnzdon444sj9eHgaCNgnNk3ckdKc5xSgyax946Uuvau",
);
pub mod constants {
    pub const SEED_ESCROW: &[u8] = b"escrow";
}
pub mod errors {
    use naclac_lang::prelude::*;
    pub enum EscrowError {
        /// Invalid mint for the trade.
        InvalidMint,
        /// Unauthorized action.
        Unauthorized,
        /// Overflow or math error.
        Overflow,
        /// The amount provided must be greater than zero.
        ZeroAmount,
    }
    #[automatically_derived]
    impl ::core::marker::Copy for EscrowError {}
    #[automatically_derived]
    #[doc(hidden)]
    unsafe impl ::core::clone::TrivialClone for EscrowError {}
    #[automatically_derived]
    impl ::core::clone::Clone for EscrowError {
        #[inline]
        fn clone(&self) -> EscrowError {
            *self
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for EscrowError {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_fields_are_eq(&self) {}
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for EscrowError {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for EscrowError {
        #[inline]
        fn eq(&self, other: &EscrowError) -> bool {
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            let __arg1_discr = ::core::intrinsics::discriminant_value(other);
            __self_discr == __arg1_discr
        }
    }
    const _: () = if !(6000u32 == naclac_lang::error::USER_ERROR_BASE) {
        {
            ::core::panicking::panic_fmt(
                format_args!(
                    "naclac-syn\'s default #[error_code] offset differs from USER_ERROR_BASE",
                ),
            );
        }
    };
    const _: () = if !(6000u32 >= naclac_lang::error::USER_ERROR_BASE
        && 6000u32 < naclac_lang::error::FRAMEWORK_ERROR_BASE)
    {
        {
            ::core::panicking::panic_fmt(
                format_args!(
                    "#[error_code] on `EscrowError`: `offset = 6000` is below naclac\'s user error range, which starts at USER_ERROR_BASE (6000)",
                ),
            );
        }
    };
    const _: () = if !((EscrowError::InvalidMint as u32)
        < naclac_lang::error::FRAMEWORK_ERROR_BASE - 6000u32)
    {
        {
            ::core::panicking::panic_fmt(
                format_args!(
                    "#[error_code] variant `EscrowError::InvalidMint` has a negative or too-large discriminant: user error codes (6000 + discriminant) must stay below 10000, where naclac\'s framework error codes begin",
                ),
            );
        }
    };
    const _: () = if !((EscrowError::Unauthorized as u32)
        < naclac_lang::error::FRAMEWORK_ERROR_BASE - 6000u32)
    {
        {
            ::core::panicking::panic_fmt(
                format_args!(
                    "#[error_code] variant `EscrowError::Unauthorized` has a negative or too-large discriminant: user error codes (6000 + discriminant) must stay below 10000, where naclac\'s framework error codes begin",
                ),
            );
        }
    };
    const _: () = if !((EscrowError::Overflow as u32)
        < naclac_lang::error::FRAMEWORK_ERROR_BASE - 6000u32)
    {
        {
            ::core::panicking::panic_fmt(
                format_args!(
                    "#[error_code] variant `EscrowError::Overflow` has a negative or too-large discriminant: user error codes (6000 + discriminant) must stay below 10000, where naclac\'s framework error codes begin",
                ),
            );
        }
    };
    const _: () = if !((EscrowError::ZeroAmount as u32)
        < naclac_lang::error::FRAMEWORK_ERROR_BASE - 6000u32)
    {
        {
            ::core::panicking::panic_fmt(
                format_args!(
                    "#[error_code] variant `EscrowError::ZeroAmount` has a negative or too-large discriminant: user error codes (6000 + discriminant) must stay below 10000, where naclac\'s framework error codes begin",
                ),
            );
        }
    };
    impl From<EscrowError> for naclac_lang::prelude::ProgramError {
        fn from(e: EscrowError) -> Self {
            naclac_lang::prelude::ProgramError::Custom(e as u32 + 6000u32)
        }
    }
}
pub mod events {
    use naclac_lang::prelude::*;
    #[bytemuck(crate = "naclac_lang::bytemuck")]
    #[repr(C, packed)]
    pub struct EscrowCreated {
        pub maker: Address,
        pub amount_a: u64,
        pub amount_b: u64,
    }
    #[automatically_derived]
    #[doc(hidden)]
    unsafe impl ::core::clone::TrivialClone for EscrowCreated {}
    #[automatically_derived]
    impl ::core::clone::Clone for EscrowCreated {
        #[inline]
        fn clone(&self) -> EscrowCreated {
            let _: ::core::clone::AssertParamIsClone<Address>;
            let _: ::core::clone::AssertParamIsClone<u64>;
            *self
        }
    }
    #[automatically_derived]
    impl ::core::marker::Copy for EscrowCreated {}
    const _: fn() = || {
        #[allow(clippy::missing_const_for_fn)]
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Pod>() {}
            assert_impl::<Address>();
        }
    };
    const _: fn() = || {
        #[allow(clippy::missing_const_for_fn)]
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Pod>() {}
            assert_impl::<u64>();
        }
    };
    const _: fn() = || {
        #[allow(clippy::missing_const_for_fn)]
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Pod>() {}
            assert_impl::<u64>();
        }
    };
    unsafe impl naclac_lang::bytemuck::Pod for EscrowCreated {}
    const _: fn() = || {
        #[allow(clippy::missing_const_for_fn)]
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Zeroable>() {}
            assert_impl::<Address>();
        }
    };
    const _: fn() = || {
        #[allow(clippy::missing_const_for_fn)]
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Zeroable>() {}
            assert_impl::<u64>();
        }
    };
    const _: fn() = || {
        #[allow(clippy::missing_const_for_fn)]
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Zeroable>() {}
            assert_impl::<u64>();
        }
    };
    unsafe impl naclac_lang::bytemuck::Zeroable for EscrowCreated {}
    const _: () = {
        if !(::core::mem::size_of::<EscrowCreated>()
            == (::core::mem::size_of::<Address>() + ::core::mem::size_of::<u64>()
                + ::core::mem::size_of::<u64>()))
        {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "defined_type/#[account]: `EscrowCreated` has a different size than its fields add up to; a zero-copy struct is `#[repr(C, packed)]` and must have no padding",
                    ),
                );
            }
        }
        if !(::core::mem::align_of::<EscrowCreated>() == 1) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "defined_type/#[account]: `EscrowCreated` must have alignment 1; a zero-copy struct is `#[repr(C, packed)]`",
                    ),
                );
            }
        }
    };
    const _: fn() = || {
        fn assert_impl<T: naclac_lang::prelude::ZcField>() {}
        assert_impl::<Address>();
        assert_impl::<u64>();
        assert_impl::<u64>();
    };
    impl Default for EscrowCreated {
        fn default() -> Self {
            Self {
                maker: <Address as naclac_lang::prelude::bytemuck::Zeroable>::zeroed(),
                amount_a: <u64 as naclac_lang::prelude::bytemuck::Zeroable>::zeroed(),
                amount_b: <u64 as naclac_lang::prelude::bytemuck::Zeroable>::zeroed(),
            }
        }
    }
    impl EscrowCreated {
        pub fn emit(&self) {
            let mut __frame = naclac_lang::event::EventFrame::<
                { 8usize + core::mem::size_of::<EscrowCreated>() },
            >::new();
            __frame.push(&[70u8, 127u8, 105u8, 102u8, 92u8, 97u8, 7u8, 173u8]);
            __frame.push(naclac_lang::prelude::bytemuck::bytes_of(self));
            naclac_lang::prelude::sol_log_data(&[__frame.finish()]);
        }
    }
    impl naclac_lang::prelude::EventCpiPayload for EscrowCreated {
        fn with_cpi_payload<R>(&self, tag: &[u8], send: impl FnOnce(&[u8]) -> R) -> R {
            let discriminator: &[u8] = &[
                70u8, 127u8, 105u8, 102u8, 92u8, 97u8, 7u8, 173u8,
            ];
            let data = naclac_lang::prelude::bytemuck::bytes_of(self);
            naclac_lang::event::with_concat(&[tag, discriminator, data], send)
        }
    }
    #[bytemuck(crate = "naclac_lang::bytemuck")]
    #[repr(C, packed)]
    pub struct EscrowExchanged {
        pub taker: Address,
        pub amount_a: u64,
        pub amount_b: u64,
    }
    #[automatically_derived]
    #[doc(hidden)]
    unsafe impl ::core::clone::TrivialClone for EscrowExchanged {}
    #[automatically_derived]
    impl ::core::clone::Clone for EscrowExchanged {
        #[inline]
        fn clone(&self) -> EscrowExchanged {
            let _: ::core::clone::AssertParamIsClone<Address>;
            let _: ::core::clone::AssertParamIsClone<u64>;
            *self
        }
    }
    #[automatically_derived]
    impl ::core::marker::Copy for EscrowExchanged {}
    const _: fn() = || {
        #[allow(clippy::missing_const_for_fn)]
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Pod>() {}
            assert_impl::<Address>();
        }
    };
    const _: fn() = || {
        #[allow(clippy::missing_const_for_fn)]
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Pod>() {}
            assert_impl::<u64>();
        }
    };
    const _: fn() = || {
        #[allow(clippy::missing_const_for_fn)]
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Pod>() {}
            assert_impl::<u64>();
        }
    };
    unsafe impl naclac_lang::bytemuck::Pod for EscrowExchanged {}
    const _: fn() = || {
        #[allow(clippy::missing_const_for_fn)]
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Zeroable>() {}
            assert_impl::<Address>();
        }
    };
    const _: fn() = || {
        #[allow(clippy::missing_const_for_fn)]
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Zeroable>() {}
            assert_impl::<u64>();
        }
    };
    const _: fn() = || {
        #[allow(clippy::missing_const_for_fn)]
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Zeroable>() {}
            assert_impl::<u64>();
        }
    };
    unsafe impl naclac_lang::bytemuck::Zeroable for EscrowExchanged {}
    const _: () = {
        if !(::core::mem::size_of::<EscrowExchanged>()
            == (::core::mem::size_of::<Address>() + ::core::mem::size_of::<u64>()
                + ::core::mem::size_of::<u64>()))
        {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "defined_type/#[account]: `EscrowExchanged` has a different size than its fields add up to; a zero-copy struct is `#[repr(C, packed)]` and must have no padding",
                    ),
                );
            }
        }
        if !(::core::mem::align_of::<EscrowExchanged>() == 1) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "defined_type/#[account]: `EscrowExchanged` must have alignment 1; a zero-copy struct is `#[repr(C, packed)]`",
                    ),
                );
            }
        }
    };
    const _: fn() = || {
        fn assert_impl<T: naclac_lang::prelude::ZcField>() {}
        assert_impl::<Address>();
        assert_impl::<u64>();
        assert_impl::<u64>();
    };
    impl Default for EscrowExchanged {
        fn default() -> Self {
            Self {
                taker: <Address as naclac_lang::prelude::bytemuck::Zeroable>::zeroed(),
                amount_a: <u64 as naclac_lang::prelude::bytemuck::Zeroable>::zeroed(),
                amount_b: <u64 as naclac_lang::prelude::bytemuck::Zeroable>::zeroed(),
            }
        }
    }
    impl EscrowExchanged {
        pub fn emit(&self) {
            let mut __frame = naclac_lang::event::EventFrame::<
                { 8usize + core::mem::size_of::<EscrowExchanged>() },
            >::new();
            __frame.push(&[227u8, 173u8, 205u8, 57u8, 1u8, 154u8, 129u8, 42u8]);
            __frame.push(naclac_lang::prelude::bytemuck::bytes_of(self));
            naclac_lang::prelude::sol_log_data(&[__frame.finish()]);
        }
    }
    impl naclac_lang::prelude::EventCpiPayload for EscrowExchanged {
        fn with_cpi_payload<R>(&self, tag: &[u8], send: impl FnOnce(&[u8]) -> R) -> R {
            let discriminator: &[u8] = &[
                227u8, 173u8, 205u8, 57u8, 1u8, 154u8, 129u8, 42u8,
            ];
            let data = naclac_lang::prelude::bytemuck::bytes_of(self);
            naclac_lang::event::with_concat(&[tag, discriminator, data], send)
        }
    }
    #[bytemuck(crate = "naclac_lang::bytemuck")]
    #[repr(C, packed)]
    pub struct EscrowCancelled {
        pub maker: Address,
        pub amount_a: u64,
    }
    #[automatically_derived]
    #[doc(hidden)]
    unsafe impl ::core::clone::TrivialClone for EscrowCancelled {}
    #[automatically_derived]
    impl ::core::clone::Clone for EscrowCancelled {
        #[inline]
        fn clone(&self) -> EscrowCancelled {
            let _: ::core::clone::AssertParamIsClone<Address>;
            let _: ::core::clone::AssertParamIsClone<u64>;
            *self
        }
    }
    #[automatically_derived]
    impl ::core::marker::Copy for EscrowCancelled {}
    const _: fn() = || {
        #[allow(clippy::missing_const_for_fn)]
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Pod>() {}
            assert_impl::<Address>();
        }
    };
    const _: fn() = || {
        #[allow(clippy::missing_const_for_fn)]
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Pod>() {}
            assert_impl::<u64>();
        }
    };
    unsafe impl naclac_lang::bytemuck::Pod for EscrowCancelled {}
    const _: fn() = || {
        #[allow(clippy::missing_const_for_fn)]
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Zeroable>() {}
            assert_impl::<Address>();
        }
    };
    const _: fn() = || {
        #[allow(clippy::missing_const_for_fn)]
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Zeroable>() {}
            assert_impl::<u64>();
        }
    };
    unsafe impl naclac_lang::bytemuck::Zeroable for EscrowCancelled {}
    const _: () = {
        if !(::core::mem::size_of::<EscrowCancelled>()
            == (::core::mem::size_of::<Address>() + ::core::mem::size_of::<u64>()))
        {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "defined_type/#[account]: `EscrowCancelled` has a different size than its fields add up to; a zero-copy struct is `#[repr(C, packed)]` and must have no padding",
                    ),
                );
            }
        }
        if !(::core::mem::align_of::<EscrowCancelled>() == 1) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "defined_type/#[account]: `EscrowCancelled` must have alignment 1; a zero-copy struct is `#[repr(C, packed)]`",
                    ),
                );
            }
        }
    };
    const _: fn() = || {
        fn assert_impl<T: naclac_lang::prelude::ZcField>() {}
        assert_impl::<Address>();
        assert_impl::<u64>();
    };
    impl Default for EscrowCancelled {
        fn default() -> Self {
            Self {
                maker: <Address as naclac_lang::prelude::bytemuck::Zeroable>::zeroed(),
                amount_a: <u64 as naclac_lang::prelude::bytemuck::Zeroable>::zeroed(),
            }
        }
    }
    impl EscrowCancelled {
        pub fn emit(&self) {
            let mut __frame = naclac_lang::event::EventFrame::<
                { 8usize + core::mem::size_of::<EscrowCancelled>() },
            >::new();
            __frame.push(&[98u8, 241u8, 195u8, 122u8, 213u8, 0u8, 162u8, 161u8]);
            __frame.push(naclac_lang::prelude::bytemuck::bytes_of(self));
            naclac_lang::prelude::sol_log_data(&[__frame.finish()]);
        }
    }
    impl naclac_lang::prelude::EventCpiPayload for EscrowCancelled {
        fn with_cpi_payload<R>(&self, tag: &[u8], send: impl FnOnce(&[u8]) -> R) -> R {
            let discriminator: &[u8] = &[
                98u8, 241u8, 195u8, 122u8, 213u8, 0u8, 162u8, 161u8,
            ];
            let data = naclac_lang::prelude::bytemuck::bytes_of(self);
            naclac_lang::event::with_concat(&[tag, discriminator, data], send)
        }
    }
}
pub mod instructions {
    pub mod cancel {
        use {
            crate::{
                constants::SEED_ESCROW, errors::EscrowError, events::EscrowCancelled,
                state::escrow_state::EscrowState,
            },
            naclac_lang::prelude::*,
        };
        #[instruction(seed:u64)]
        pub struct Cancel {
            #[account(mut)]
            pub maker: Signer,
            pub mint_a: Account<Mint>,
            #[account(
                mut,
                seeds = [SEED_ESCROW,
                maker.address().as_ref(),
                &seed.to_le_bytes()],
                bump = escrow_state.bump,
                close = maker,
                mint_a = mint_a@EscrowError::InvalidMint,
            )]
            pub escrow_state: Account<EscrowState>,
            #[account(mut, token::mint = mint_a, token::authority = escrow_state)]
            pub vault_token_account: Account<TokenAccount>,
            #[account(mut, token::mint = mint_a, token::authority = maker)]
            pub maker_token_account_a: Account<TokenAccount>,
            pub token_program: Program<Token>,
            pub system_program: Program<System>,
        }
        pub struct CancelBumps {
            pub escrow_state: u8,
        }
        #[automatically_derived]
        #[doc(hidden)]
        unsafe impl ::core::clone::TrivialClone for CancelBumps {}
        #[automatically_derived]
        impl ::core::clone::Clone for CancelBumps {
            #[inline]
            fn clone(&self) -> CancelBumps {
                let _: ::core::clone::AssertParamIsClone<u8>;
                *self
            }
        }
        #[automatically_derived]
        impl ::core::marker::Copy for CancelBumps {}
        #[automatically_derived]
        impl ::core::default::Default for CancelBumps {
            #[inline]
            fn default() -> CancelBumps {
                CancelBumps {
                    escrow_state: ::core::default::Default::default(),
                }
            }
        }
        impl naclac_lang::prelude::EventCpiAccounts for Cancel {
            const NEEDS_EVENT_CPI: bool = false;
        }
        impl Cancel {
            pub fn to_account_metas(
                &self,
            ) -> [naclac_lang::prelude::AccountMeta; 7usize] {
                [
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.maker).address(),
                        is_signer: <Signer as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.mint_a).address(),
                        is_signer: <Account<
                            Mint,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: false,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.escrow_state).address(),
                        is_signer: <Account<
                            EscrowState,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.vault_token_account).address(),
                        is_signer: <Account<
                            TokenAccount,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.maker_token_account_a).address(),
                        is_signer: <Account<
                            TokenAccount,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.token_program).address(),
                        is_signer: <Program<
                            Token,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: false,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.system_program).address(),
                        is_signer: <Program<
                            System,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: false,
                    },
                ]
            }
        }
        impl Cancel {
            pub fn to_account_infos(
                &self,
            ) -> [Option<naclac_lang::prelude::AccountInfo>; 7usize] {
                [
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(&self.maker),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.mint_a,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.escrow_state,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.vault_token_account,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.maker_token_account_a,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.token_program,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.system_program,
                        ),
                    ),
                ]
            }
        }
        impl naclac_lang::prelude::Bumps for Cancel {
            type BumpsStruct = CancelBumps;
        }
        impl Cancel {
            pub const MUT_MASK: [u64; 4] = {
                let mut __mask = [0u64; 4];
                __mask = naclac_lang::prelude::mut_mask_set_bit(__mask, 0usize);
                __mask = naclac_lang::prelude::mut_mask_set_bit(__mask, 2usize);
                __mask = naclac_lang::prelude::mut_mask_set_bit(__mask, 3usize);
                __mask = naclac_lang::prelude::mut_mask_set_bit(__mask, 4usize);
                __mask
            };
            /// Loads and validates the accounts provided to the instruction.
            ///
            /// Walks the serialized accounts at `__input` itself: each declared
            /// field's signer/writable/executable requirement is checked as the
            /// account is walked, then any trailing accounts are walked into the
            /// remaining-accounts slice. Rejects more than 64 account entries
            /// (the capacity of `__lookup`) and fewer than this struct declares.
            ///
            /// # Safety
            ///
            /// `__input` must be the runtime-provided entrypoint input buffer and
            /// `__num_accounts` the account count stored at its start.
            pub unsafe fn load_and_validate<const __NACLAC_IX_DISC_LEN: usize>(
                program_id: &'static naclac_lang::prelude::Address,
                __input: *mut u8,
                __num_accounts: usize,
                __lookup: &'static mut [core::mem::MaybeUninit<
                    naclac_lang::prelude::AccountView,
                >; 64],
                instruction_data: &[u8],
            ) -> Result<
                (Self, &'static [naclac_lang::prelude::AccountType], CancelBumps),
                naclac_lang::prelude::ProgramError,
            > {
                if __num_accounts.wrapping_sub(7usize) > 57usize {
                    return Err(
                        naclac_lang::prelude::account_count_error(__num_accounts),
                    );
                }
                let __lookup_ptr = __lookup.as_mut_ptr()
                    as *mut naclac_lang::prelude::AccountView;
                let mut __cursor = unsafe {
                    naclac_lang::prelude::AccountCursor::new(
                        __input,
                        __lookup_ptr,
                        program_id,
                    )
                };
                const __DECLARED_CHECKS: [(u32, u32, bool); 7usize] = [
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Signer as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            true,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Account<
                                Mint,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            false,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Account<
                                EscrowState,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            true,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Account<
                                TokenAccount,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            true,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Account<
                                TokenAccount,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            true,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Program<
                                Token,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            false,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Program<
                                System,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            false,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                ];
                let mut __declared = [core::mem::MaybeUninit::<
                    naclac_lang::prelude::AccountView,
                >::uninit(); 7usize];
                if !unsafe {
                    __cursor
                        .try_declared_fast(
                            __DECLARED_CHECKS[0].0,
                            __DECLARED_CHECKS[0].1,
                        )
                        .is_some_and(|__view| {
                            __declared[0].write(__view);
                            true
                        })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[1].0,
                                __DECLARED_CHECKS[1].1,
                            )
                            .is_some_and(|__view| {
                                __declared[1].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[2].0,
                                __DECLARED_CHECKS[2].1,
                            )
                            .is_some_and(|__view| {
                                __declared[2].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[3].0,
                                __DECLARED_CHECKS[3].1,
                            )
                            .is_some_and(|__view| {
                                __declared[3].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[4].0,
                                __DECLARED_CHECKS[4].1,
                            )
                            .is_some_and(|__view| {
                                __declared[4].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[5].0,
                                __DECLARED_CHECKS[5].1,
                            )
                            .is_some_and(|__view| {
                                __declared[5].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[6].0,
                                __DECLARED_CHECKS[6].1,
                            )
                            .is_some_and(|__view| {
                                __declared[6].write(__view);
                                true
                            })
                } || __num_accounts > 7usize
                {
                    unsafe {
                        __cursor
                            .finish_walk_slow(
                                __input,
                                &__DECLARED_CHECKS,
                                __num_accounts,
                                &Self::MUT_MASK,
                            )
                    }?;
                    unsafe {
                        __declared[0].write(*__lookup_ptr.add(0));
                        __declared[1].write(*__lookup_ptr.add(1));
                        __declared[2].write(*__lookup_ptr.add(2));
                        __declared[3].write(*__lookup_ptr.add(3));
                        __declared[4].write(*__lookup_ptr.add(4));
                        __declared[5].write(*__lookup_ptr.add(5));
                        __declared[6].write(*__lookup_ptr.add(6));
                    }
                }
                let __declared_infos = unsafe {
                    core::mem::transmute::<
                        [core::mem::MaybeUninit<
                            naclac_lang::prelude::AccountView,
                        >; 7usize],
                        [naclac_lang::prelude::AccountType; 7usize],
                    >(__declared)
                };
                let __views = &__declared_infos;
                let accounts = __views;
                let mut __ix_offset: usize = __NACLAC_IX_DISC_LEN;
                let seed = <u64 as naclac_lang::prelude::NaclacArgs>::naclac_deserialize(
                    instruction_data,
                    &mut __ix_offset,
                )?;
                let _ = &seed;
                let info = &__views[0usize];
                if <Signer as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(0usize),
                        );
                    }
                }
                let mut maker = {
                    const _: () = if !<Signer as naclac_lang::prelude::NaclacAccount>::IS_SIGNER {
                        {
                            ::core::panicking::panic_fmt(
                                format_args!(
                                    "a signer slot built without a signer check must be a type with IS_SIGNER",
                                ),
                            );
                        }
                    };
                    let __loaded = unsafe { <Signer>::from_verified_signer(info) };
                    __loaded
                };
                let info = &__views[1usize];
                if <Account<
                    Mint,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(1usize),
                        );
                    }
                }
                let mut mint_a = <Account<
                    Mint,
                > as naclac_lang::prelude::NaclacAccount>::try_from(info, 1usize)?;
                let info = &__views[2usize];
                if <Account<
                    EscrowState,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(2usize),
                        );
                    }
                }
                let mut escrow_state = <Account<
                    EscrowState,
                > as naclac_lang::prelude::NaclacAccount>::try_from_mut(info, 2usize)?;
                let __seed_escrow_state_0 = &SEED_ESCROW;
                let __seed_escrow_state_1 = naclac_lang::prelude::ToAddress::address(
                    &(maker.address()),
                );
                let __seed_escrow_state_2 = &&seed.to_le_bytes();
                let expected_bump = (escrow_state.bump);
                let __pda_program: &naclac_lang::prelude::Address = program_id;
                let __seeds: &[&[u8]] = &[
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_escrow_state_0,
                    ),
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_escrow_state_1,
                    ),
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_escrow_state_2,
                    ),
                ];
                let expected_pda = naclac_lang::prelude::derive_program_address(
                    __seeds,
                    expected_bump,
                    __pda_program,
                );
                {
                    let __actual_address: &naclac_lang::prelude::Address = naclac_lang::prelude::Address::from_address_ref(
                        info.view.address(),
                    );
                    if *__actual_address != expected_pda {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintSeeds
                                .err(2usize),
                        );
                    }
                }
                if expected_bump != (escrow_state.bump) {
                    return Err(
                        naclac_lang::prelude::NaclacError::ConstraintSeeds.err(2usize),
                    );
                }
                let __bump_escrow_state = expected_bump;
                let info = &__views[3usize];
                if <Account<
                    TokenAccount,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(3usize),
                        );
                    }
                }
                let mut vault_token_account = <Account<
                    TokenAccount,
                > as naclac_lang::prelude::NaclacAccount>::try_from_mut(info, 3usize)?;
                {
                    const _: () = naclac_lang::token_constraints::token::RULES
                        .check(
                            &["mint", "authority"],
                            naclac_lang::prelude::ConstraintPhase::Check,
                        );
                    <naclac_lang::token_constraints::token::Constraint as naclac_lang::prelude::AccountConstraint<
                        _,
                    >>::check(
                            &vault_token_account,
                            &<naclac_lang::token_constraints::token::Args as ::core::default::Default>::default()
                                .mint(
                                    naclac_lang::prelude::ConstraintValue::into_value(
                                        &accounts[1usize],
                                    ),
                                )
                                .authority(
                                    naclac_lang::prelude::ConstraintValue::into_value(
                                        &accounts[2usize],
                                    ),
                                ),
                        )
                        .map_err(|e| e.err(3usize))?;
                }
                let info = &__views[4usize];
                if <Account<
                    TokenAccount,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(4usize),
                        );
                    }
                }
                let mut maker_token_account_a = <Account<
                    TokenAccount,
                > as naclac_lang::prelude::NaclacAccount>::try_from_mut(info, 4usize)?;
                {
                    const _: () = naclac_lang::token_constraints::token::RULES
                        .check(
                            &["mint", "authority"],
                            naclac_lang::prelude::ConstraintPhase::Check,
                        );
                    <naclac_lang::token_constraints::token::Constraint as naclac_lang::prelude::AccountConstraint<
                        _,
                    >>::check(
                            &maker_token_account_a,
                            &<naclac_lang::token_constraints::token::Args as ::core::default::Default>::default()
                                .mint(
                                    naclac_lang::prelude::ConstraintValue::into_value(
                                        &accounts[1usize],
                                    ),
                                )
                                .authority(
                                    naclac_lang::prelude::ConstraintValue::into_value(&maker),
                                ),
                        )
                        .map_err(|e| e.err(4usize))?;
                }
                let info = &__views[5usize];
                const _: () = if !<Program<
                    Token,
                > as naclac_lang::prelude::NaclacAccount>::CHECKS_PROGRAM_ID {
                    {
                        ::core::panicking::panic_fmt(
                            format_args!(
                                "no address check is emitted for this field, so its type must check the program id",
                            ),
                        );
                    }
                };
                if <Program<
                    Token,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(5usize),
                        );
                    }
                }
                let mut token_program = <Program<
                    Token,
                > as naclac_lang::prelude::NaclacAccount>::try_from(info, 5usize)?;
                let info = &__views[6usize];
                const _: () = if !<Program<
                    System,
                > as naclac_lang::prelude::NaclacAccount>::CHECKS_PROGRAM_ID {
                    {
                        ::core::panicking::panic_fmt(
                            format_args!(
                                "no address check is emitted for this field, so its type must check the program id",
                            ),
                        );
                    }
                };
                if <Program<
                    System,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(6usize),
                        );
                    }
                }
                let mut system_program = <Program<
                    System,
                > as naclac_lang::prelude::NaclacAccount>::try_from(info, 6usize)?;
                let __remaining: &'static [naclac_lang::prelude::AccountType] = unsafe {
                    core::slice::from_raw_parts(
                        (__lookup_ptr as *const naclac_lang::prelude::AccountType)
                            .add(7usize),
                        __num_accounts.wrapping_sub(7usize),
                    )
                };
                let mut __me = Self {
                    maker,
                    mint_a,
                    escrow_state,
                    vault_token_account,
                    maker_token_account_a,
                    token_program,
                    system_program,
                };
                let __val = __me.escrow_state.mint_a;
                let __target_key = (__me.mint_a).address();
                if __val != __target_key {
                    return Err(EscrowError::InvalidMint.into());
                }
                Ok((
                    __me,
                    __remaining,
                    CancelBumps {
                        escrow_state: __bump_escrow_state,
                    },
                ))
            }
            /// Persists mutations and handles cleanup.
            ///
            /// For zero-copy accounts this is a no-op (`Ok(())`). Marked `#[inline(always)]`
            /// so the compiler eliminates the call and the conditional branch in the dispatcher
            /// when the body is empty.
            #[inline(always)]
            pub fn teardown(
                &mut self,
                program_id: &naclac_lang::prelude::Address,
                bumps: &CancelBumps,
            ) -> naclac_lang::prelude::Result {
                let __target = &self.escrow_state;
                if naclac_lang::prelude::ToAddress::address(__target)
                    == naclac_lang::prelude::ToAddress::address(&self.maker)
                {
                    return Err(
                        naclac_lang::prelude::NaclacError::ConstraintClose.err(2usize),
                    );
                }
                {
                    let target_info = naclac_lang::prelude::ToAccountInfo::to_account_info(
                        __target,
                    );
                    let dest_info = naclac_lang::prelude::ToAccountInfo::to_account_info(
                        &self.maker,
                    );
                    if target_info.owner() != *program_id {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(2usize),
                        );
                    }
                    let target_lamports = target_info.lamports();
                    dest_info.add_lamports(target_lamports)?;
                    target_info.sub_lamports(target_lamports)?;
                    let mut target_view = target_info.view;
                    unsafe {
                        target_view
                            .assign(
                                naclac_lang::prelude::SYSTEM_PROGRAM_ID.as_address(),
                            );
                        core::ptr::write_bytes(
                            target_view.data_ptr() as *mut u8,
                            0,
                            target_view.data_len(),
                        );
                    }
                }
                Ok(())
            }
        }
        pub fn cancel(ctx: Context<Cancel>, seed: u64) -> Result {
            let maker_addr = ctx.accounts.maker.address();
            let seed_bytes = seed.to_le_bytes();
            let seeds: &[&[u8]] = &[
                SEED_ESCROW,
                maker_addr.as_ref(),
                &seed_bytes,
                &[ctx.accounts.escrow_state.bump],
            ];
            let signer_seeds: &[&[&[u8]]] = &[seeds];
            let decimals = ctx.accounts.mint_a.decimals();
            ctx.accounts
                .token_program
                .transfer_checked_signed(
                    TransferCheckedAccounts {
                        from: &mut ctx.accounts.vault_token_account,
                        mint: &ctx.accounts.mint_a,
                        to: &mut ctx.accounts.maker_token_account_a,
                        authority: &ctx.accounts.escrow_state,
                    },
                    ctx.accounts.escrow_state.amount_a,
                    decimals,
                    signer_seeds,
                )?;
            ctx.accounts
                .token_program
                .close_account_signed(
                    CloseAccountAccounts {
                        account: &mut ctx.accounts.vault_token_account,
                        destination: &mut ctx.accounts.maker,
                        authority: &ctx.accounts.escrow_state,
                    },
                    signer_seeds,
                )?;
            {
                let mut __event = <EscrowCancelled as core::default::Default>::default();
                __event.maker = (ctx.accounts.maker.address()).into();
                __event.amount_a = (ctx.accounts.escrow_state.amount_a).into();
                __event.emit();
                __event
            };
            Ok(())
        }
    }
    pub mod make {
        use {
            crate::{
                constants::SEED_ESCROW, errors::EscrowError, events::EscrowCreated,
                state::escrow_state::EscrowState,
            },
            naclac_lang::prelude::*,
        };
        #[instruction(seed:u64)]
        pub struct Make {
            #[account(mut)]
            pub maker: Signer,
            pub mint_a: Account<Mint>,
            pub mint_b: Account<Mint>,
            #[account(
                init,
                payer = maker,
                seeds = [SEED_ESCROW,
                maker.address().as_ref(),
                &seed.to_le_bytes()],
                bump
            )]
            pub escrow_state: Account<EscrowState>,
            #[account(mut, token::mint = mint_a, token::authority = escrow_state)]
            pub vault_token_account: Account<TokenAccount>,
            #[account(mut, token::mint = mint_a, token::authority = maker)]
            pub maker_token_account_a: Account<TokenAccount>,
            pub token_program: Program<Token>,
            pub system_program: Program<System>,
        }
        pub struct MakeBumps {
            pub escrow_state: u8,
        }
        #[automatically_derived]
        #[doc(hidden)]
        unsafe impl ::core::clone::TrivialClone for MakeBumps {}
        #[automatically_derived]
        impl ::core::clone::Clone for MakeBumps {
            #[inline]
            fn clone(&self) -> MakeBumps {
                let _: ::core::clone::AssertParamIsClone<u8>;
                *self
            }
        }
        #[automatically_derived]
        impl ::core::marker::Copy for MakeBumps {}
        #[automatically_derived]
        impl ::core::default::Default for MakeBumps {
            #[inline]
            fn default() -> MakeBumps {
                MakeBumps {
                    escrow_state: ::core::default::Default::default(),
                }
            }
        }
        impl naclac_lang::prelude::EventCpiAccounts for Make {
            const NEEDS_EVENT_CPI: bool = false;
        }
        impl Make {
            pub fn to_account_metas(
                &self,
            ) -> [naclac_lang::prelude::AccountMeta; 8usize] {
                [
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.maker).address(),
                        is_signer: <Signer as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.mint_a).address(),
                        is_signer: <Account<
                            Mint,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: false,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.mint_b).address(),
                        is_signer: <Account<
                            Mint,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: false,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.escrow_state).address(),
                        is_signer: <Account<
                            EscrowState,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.vault_token_account).address(),
                        is_signer: <Account<
                            TokenAccount,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.maker_token_account_a).address(),
                        is_signer: <Account<
                            TokenAccount,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.token_program).address(),
                        is_signer: <Program<
                            Token,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: false,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.system_program).address(),
                        is_signer: <Program<
                            System,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: false,
                    },
                ]
            }
        }
        impl Make {
            pub fn to_account_infos(
                &self,
            ) -> [Option<naclac_lang::prelude::AccountInfo>; 8usize] {
                [
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(&self.maker),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.mint_a,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.mint_b,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.escrow_state,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.vault_token_account,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.maker_token_account_a,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.token_program,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.system_program,
                        ),
                    ),
                ]
            }
        }
        impl naclac_lang::prelude::Bumps for Make {
            type BumpsStruct = MakeBumps;
        }
        impl Make {
            pub const MUT_MASK: [u64; 4] = {
                let mut __mask = [0u64; 4];
                __mask = naclac_lang::prelude::mut_mask_set_bit(__mask, 0usize);
                __mask = naclac_lang::prelude::mut_mask_set_bit(__mask, 3usize);
                __mask = naclac_lang::prelude::mut_mask_set_bit(__mask, 4usize);
                __mask = naclac_lang::prelude::mut_mask_set_bit(__mask, 5usize);
                __mask
            };
            /// Loads and validates the accounts provided to the instruction.
            ///
            /// Walks the serialized accounts at `__input` itself: each declared
            /// field's signer/writable/executable requirement is checked as the
            /// account is walked, then any trailing accounts are walked into the
            /// remaining-accounts slice. Rejects more than 64 account entries
            /// (the capacity of `__lookup`) and fewer than this struct declares.
            ///
            /// # Safety
            ///
            /// `__input` must be the runtime-provided entrypoint input buffer and
            /// `__num_accounts` the account count stored at its start.
            pub unsafe fn load_and_validate<const __NACLAC_IX_DISC_LEN: usize>(
                program_id: &'static naclac_lang::prelude::Address,
                __input: *mut u8,
                __num_accounts: usize,
                __lookup: &'static mut [core::mem::MaybeUninit<
                    naclac_lang::prelude::AccountView,
                >; 64],
                instruction_data: &[u8],
            ) -> Result<
                (Self, &'static [naclac_lang::prelude::AccountType], MakeBumps),
                naclac_lang::prelude::ProgramError,
            > {
                if __num_accounts.wrapping_sub(8usize) > 56usize {
                    return Err(
                        naclac_lang::prelude::account_count_error(__num_accounts),
                    );
                }
                let __lookup_ptr = __lookup.as_mut_ptr()
                    as *mut naclac_lang::prelude::AccountView;
                let mut __cursor = unsafe {
                    naclac_lang::prelude::AccountCursor::new(
                        __input,
                        __lookup_ptr,
                        program_id,
                    )
                };
                const __DECLARED_CHECKS: [(u32, u32, bool); 8usize] = [
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Signer as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            true,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Account<
                                Mint,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            false,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Account<
                                Mint,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            false,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Account<
                                EscrowState,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            true,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Account<
                                TokenAccount,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            true,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Account<
                                TokenAccount,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            true,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Program<
                                Token,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            false,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Program<
                                System,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            false,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                ];
                let mut __declared = [core::mem::MaybeUninit::<
                    naclac_lang::prelude::AccountView,
                >::uninit(); 8usize];
                if !unsafe {
                    __cursor
                        .try_declared_fast(
                            __DECLARED_CHECKS[0].0,
                            __DECLARED_CHECKS[0].1,
                        )
                        .is_some_and(|__view| {
                            __declared[0].write(__view);
                            true
                        })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[1].0,
                                __DECLARED_CHECKS[1].1,
                            )
                            .is_some_and(|__view| {
                                __declared[1].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[2].0,
                                __DECLARED_CHECKS[2].1,
                            )
                            .is_some_and(|__view| {
                                __declared[2].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[3].0,
                                __DECLARED_CHECKS[3].1,
                            )
                            .is_some_and(|__view| {
                                __declared[3].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[4].0,
                                __DECLARED_CHECKS[4].1,
                            )
                            .is_some_and(|__view| {
                                __declared[4].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[5].0,
                                __DECLARED_CHECKS[5].1,
                            )
                            .is_some_and(|__view| {
                                __declared[5].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[6].0,
                                __DECLARED_CHECKS[6].1,
                            )
                            .is_some_and(|__view| {
                                __declared[6].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[7].0,
                                __DECLARED_CHECKS[7].1,
                            )
                            .is_some_and(|__view| {
                                __declared[7].write(__view);
                                true
                            })
                } || __num_accounts > 8usize
                {
                    unsafe {
                        __cursor
                            .finish_walk_slow(
                                __input,
                                &__DECLARED_CHECKS,
                                __num_accounts,
                                &Self::MUT_MASK,
                            )
                    }?;
                    unsafe {
                        __declared[0].write(*__lookup_ptr.add(0));
                        __declared[1].write(*__lookup_ptr.add(1));
                        __declared[2].write(*__lookup_ptr.add(2));
                        __declared[3].write(*__lookup_ptr.add(3));
                        __declared[4].write(*__lookup_ptr.add(4));
                        __declared[5].write(*__lookup_ptr.add(5));
                        __declared[6].write(*__lookup_ptr.add(6));
                        __declared[7].write(*__lookup_ptr.add(7));
                    }
                }
                let __declared_infos = unsafe {
                    core::mem::transmute::<
                        [core::mem::MaybeUninit<
                            naclac_lang::prelude::AccountView,
                        >; 8usize],
                        [naclac_lang::prelude::AccountType; 8usize],
                    >(__declared)
                };
                let __views = &__declared_infos;
                let accounts = __views;
                let mut __ix_offset: usize = __NACLAC_IX_DISC_LEN;
                let seed = <u64 as naclac_lang::prelude::NaclacArgs>::naclac_deserialize(
                    instruction_data,
                    &mut __ix_offset,
                )?;
                let _ = &seed;
                let info = &__views[0usize];
                if <Signer as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(0usize),
                        );
                    }
                }
                let mut maker = {
                    const _: () = if !<Signer as naclac_lang::prelude::NaclacAccount>::IS_SIGNER {
                        {
                            ::core::panicking::panic_fmt(
                                format_args!(
                                    "a signer slot built without a signer check must be a type with IS_SIGNER",
                                ),
                            );
                        }
                    };
                    let __loaded = unsafe { <Signer>::from_verified_signer(info) };
                    __loaded
                };
                let info = &__views[1usize];
                if <Account<
                    Mint,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(1usize),
                        );
                    }
                }
                let mut mint_a = <Account<
                    Mint,
                > as naclac_lang::prelude::NaclacAccount>::try_from(info, 1usize)?;
                let info = &__views[2usize];
                if <Account<
                    Mint,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(2usize),
                        );
                    }
                }
                let mut mint_b = <Account<
                    Mint,
                > as naclac_lang::prelude::NaclacAccount>::try_from(info, 2usize)?;
                let info = &__views[3usize];
                let __seed_escrow_state_0 = &SEED_ESCROW;
                let __seed_escrow_state_1 = naclac_lang::prelude::ToAddress::address(
                    &(maker.address()),
                );
                let __seed_escrow_state_2 = &&seed.to_le_bytes();
                let __pda_program: &naclac_lang::prelude::Address = program_id;
                let __seeds: &[&[u8]] = &[
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_escrow_state_0,
                    ),
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_escrow_state_1,
                    ),
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_escrow_state_2,
                    ),
                ];
                let (expected_pda, expected_bump) = naclac_lang::prelude::find_program_address(
                        __seeds,
                        __pda_program,
                    )
                    .map_err(|_| {
                        naclac_lang::prelude::NaclacError::ConstraintSeeds.err(3usize)
                    })?;
                {
                    let __actual_address: &naclac_lang::prelude::Address = naclac_lang::prelude::Address::from_address_ref(
                        info.view.address(),
                    );
                    if *__actual_address != expected_pda {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintSeeds
                                .err(3usize),
                        );
                    }
                }
                let __bump_escrow_state = expected_bump;
                let __disc_len: usize = <EscrowState as naclac_lang::prelude::Discriminator>::DISCRIMINATOR
                    .len();
                let __needs_init = if info.data_is_empty() {
                    true
                } else {
                    {
                        let __data_ptr = info.view.data_ptr();
                        let __data_len = info.view.data_len();
                        if __disc_len == 8 {
                            __data_len >= 8
                                && unsafe { *(__data_ptr as *const u64) } == 0
                        } else {
                            __disc_len > 0 && __data_len >= __disc_len
                                && (0..__disc_len)
                                    .all(|__i| unsafe { *__data_ptr.add(__i) } == 0)
                        }
                    }
                };
                if !__needs_init {
                    return Err(
                        naclac_lang::prelude::NaclacError::AccountAlreadyInitialized
                            .err(3usize),
                    );
                }
                if __needs_init {
                    {
                        let __expected_bump_arr = [expected_bump];
                        let __seed_escrow_state_0 = &SEED_ESCROW;
                        let __seed_escrow_state_1 = naclac_lang::prelude::ToAddress::address(
                            &(maker.address()),
                        );
                        let __seed_escrow_state_2 = &&seed.to_le_bytes();
                        let pinocchio_seeds = [
                            naclac_lang::prelude::pinocchio::cpi::Seed::from(
                                naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                                    &__seed_escrow_state_0,
                                ),
                            ),
                            naclac_lang::prelude::pinocchio::cpi::Seed::from(
                                naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                                    &__seed_escrow_state_1,
                                ),
                            ),
                            naclac_lang::prelude::pinocchio::cpi::Seed::from(
                                naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                                    &__seed_escrow_state_2,
                                ),
                            ),
                            naclac_lang::prelude::pinocchio::cpi::Seed::from(
                                &__expected_bump_arr[..],
                            ),
                        ];
                        let __pinocchio_signer = naclac_lang::prelude::pinocchio::cpi::Signer::from(
                            &pinocchio_seeds,
                        );
                        let pinocchio_signer_seeds: &[naclac_lang::prelude::pinocchio::cpi::Signer] = &[
                            __pinocchio_signer,
                        ];
                        let __sys_info = &accounts[7usize];
                        if __sys_info.address()
                            != naclac_lang::prelude::SYSTEM_PROGRAM_ID
                        {
                            return Err(
                                naclac_lang::prelude::NaclacError::ProgramIdMismatch
                                    .err(7usize),
                            );
                        }
                        let __payer_info = &accounts[0usize];
                        if info.data_is_empty() {
                            let __space = ((EscrowState::SPACE)) as usize;
                            let __lamports = naclac_lang::prelude::const_rent_lamports(
                                __space,
                            );
                            naclac_lang::prelude::system_program::create_account_checked_for_init(
                                &__payer_info.view,
                                &info.view,
                                __lamports,
                                __space as u64,
                                program_id,
                                pinocchio_signer_seeds,
                            )?;
                        }
                        unsafe {
                            let ptr = info.view.data_ptr() as *mut u8;
                            let slice = core::slice::from_raw_parts_mut(ptr, __disc_len);
                            slice
                                .copy_from_slice(
                                    <EscrowState as naclac_lang::prelude::Discriminator>::DISCRIMINATOR,
                                );
                        }
                    }
                }
                let mut escrow_state = <Account<
                    EscrowState,
                > as naclac_lang::prelude::NaclacAccount>::try_from(info, 3usize)?;
                let info = &__views[4usize];
                if <Account<
                    TokenAccount,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(4usize),
                        );
                    }
                }
                let mut vault_token_account = <Account<
                    TokenAccount,
                > as naclac_lang::prelude::NaclacAccount>::try_from_mut(info, 4usize)?;
                {
                    const _: () = naclac_lang::token_constraints::token::RULES
                        .check(
                            &["mint", "authority"],
                            naclac_lang::prelude::ConstraintPhase::Check,
                        );
                    <naclac_lang::token_constraints::token::Constraint as naclac_lang::prelude::AccountConstraint<
                        _,
                    >>::check(
                            &vault_token_account,
                            &<naclac_lang::token_constraints::token::Args as ::core::default::Default>::default()
                                .mint(
                                    naclac_lang::prelude::ConstraintValue::into_value(
                                        &accounts[1usize],
                                    ),
                                )
                                .authority(
                                    naclac_lang::prelude::ConstraintValue::into_value(
                                        &accounts[3usize],
                                    ),
                                ),
                        )
                        .map_err(|e| e.err(4usize))?;
                }
                let info = &__views[5usize];
                if <Account<
                    TokenAccount,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(5usize),
                        );
                    }
                }
                let mut maker_token_account_a = <Account<
                    TokenAccount,
                > as naclac_lang::prelude::NaclacAccount>::try_from_mut(info, 5usize)?;
                {
                    const _: () = naclac_lang::token_constraints::token::RULES
                        .check(
                            &["mint", "authority"],
                            naclac_lang::prelude::ConstraintPhase::Check,
                        );
                    <naclac_lang::token_constraints::token::Constraint as naclac_lang::prelude::AccountConstraint<
                        _,
                    >>::check(
                            &maker_token_account_a,
                            &<naclac_lang::token_constraints::token::Args as ::core::default::Default>::default()
                                .mint(
                                    naclac_lang::prelude::ConstraintValue::into_value(
                                        &accounts[1usize],
                                    ),
                                )
                                .authority(
                                    naclac_lang::prelude::ConstraintValue::into_value(&maker),
                                ),
                        )
                        .map_err(|e| e.err(5usize))?;
                }
                let info = &__views[6usize];
                const _: () = if !<Program<
                    Token,
                > as naclac_lang::prelude::NaclacAccount>::CHECKS_PROGRAM_ID {
                    {
                        ::core::panicking::panic_fmt(
                            format_args!(
                                "no address check is emitted for this field, so its type must check the program id",
                            ),
                        );
                    }
                };
                if <Program<
                    Token,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(6usize),
                        );
                    }
                }
                let mut token_program = <Program<
                    Token,
                > as naclac_lang::prelude::NaclacAccount>::try_from(info, 6usize)?;
                let info = &__views[7usize];
                const _: () = if !<Program<
                    System,
                > as naclac_lang::prelude::NaclacAccount>::CHECKS_PROGRAM_ID {
                    {
                        ::core::panicking::panic_fmt(
                            format_args!(
                                "no address check is emitted for this field, so its type must check the program id",
                            ),
                        );
                    }
                };
                if <Program<
                    System,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(7usize),
                        );
                    }
                }
                let mut system_program = <Program<
                    System,
                > as naclac_lang::prelude::NaclacAccount>::try_from(info, 7usize)?;
                let __remaining: &'static [naclac_lang::prelude::AccountType] = unsafe {
                    core::slice::from_raw_parts(
                        (__lookup_ptr as *const naclac_lang::prelude::AccountType)
                            .add(8usize),
                        __num_accounts.wrapping_sub(8usize),
                    )
                };
                let mut __me = Self {
                    maker,
                    mint_a,
                    mint_b,
                    escrow_state,
                    vault_token_account,
                    maker_token_account_a,
                    token_program,
                    system_program,
                };
                Ok((
                    __me,
                    __remaining,
                    MakeBumps {
                        escrow_state: __bump_escrow_state,
                    },
                ))
            }
            /// Persists mutations and handles cleanup.
            ///
            /// For zero-copy accounts this is a no-op (`Ok(())`). Marked `#[inline(always)]`
            /// so the compiler eliminates the call and the conditional branch in the dispatcher
            /// when the body is empty.
            #[inline(always)]
            pub fn teardown(
                &mut self,
                program_id: &naclac_lang::prelude::Address,
                bumps: &MakeBumps,
            ) -> naclac_lang::prelude::Result {
                {
                    self.escrow_state.bump = bumps.escrow_state;
                }
                Ok(())
            }
        }
        pub fn make(
            ctx: Context<Make>,
            _seed: u64,
            amount_a: u64,
            amount_b: u64,
        ) -> Result {
            if !(amount_a > 0 && amount_b > 0) {
                return Err((EscrowError::ZeroAmount).into());
            }
            if !(ctx.accounts.mint_a.address() != ctx.accounts.mint_b.address()) {
                return Err((EscrowError::InvalidMint).into());
            }
            let escrow = &mut ctx.accounts.escrow_state;
            escrow.maker = ctx.accounts.maker.address();
            escrow.mint_a = ctx.accounts.mint_a.address();
            escrow.mint_b = ctx.accounts.mint_b.address();
            escrow.amount_a = amount_a;
            escrow.amount_b = amount_b;
            escrow.bump = ctx.bumps.escrow_state;
            let decimals = ctx.accounts.mint_a.decimals();
            ctx.accounts
                .token_program
                .transfer_checked(
                    TransferCheckedAccounts {
                        from: &mut ctx.accounts.maker_token_account_a,
                        mint: &ctx.accounts.mint_a,
                        to: &mut ctx.accounts.vault_token_account,
                        authority: &ctx.accounts.maker,
                    },
                    amount_a,
                    decimals,
                )?;
            {
                let mut __event = <EscrowCreated as core::default::Default>::default();
                __event.maker = (ctx.accounts.maker.address()).into();
                __event.amount_a = (amount_a).into();
                __event.amount_b = (amount_b).into();
                __event.emit();
                __event
            };
            Ok(())
        }
    }
    pub mod take {
        use {
            crate::{
                constants::SEED_ESCROW, errors::EscrowError, events::EscrowExchanged,
                state::escrow_state::EscrowState,
            },
            naclac_lang::prelude::*,
        };
        #[instruction(seed:u64)]
        pub struct Take {
            #[account(mut)]
            pub taker: Signer,
            /// SAFETY: Verified through `escrow_state` PDA seed derivation; receives rent on close.
            #[account(mut)]
            pub maker: AccountInfo,
            pub mint_a: Account<Mint>,
            pub mint_b: Account<Mint>,
            #[account(
                mut,
                seeds = [SEED_ESCROW,
                maker.address().as_ref(),
                &seed.to_le_bytes()],
                bump = escrow_state.bump,
                close = maker,
                mint_a = mint_a@EscrowError::InvalidMint,
                mint_b = mint_b@EscrowError::InvalidMint,
            )]
            pub escrow_state: Account<EscrowState>,
            #[account(mut, token::mint = mint_a, token::authority = escrow_state)]
            pub vault_token_account: Account<TokenAccount>,
            #[account(mut, token::mint = mint_a, token::authority = taker)]
            pub taker_token_account_a: Account<TokenAccount>,
            #[account(mut, token::mint = mint_b, token::authority = taker)]
            pub taker_token_account_b: Account<TokenAccount>,
            #[account(mut, token::mint = mint_b, token::authority = maker)]
            pub maker_token_account_b: Account<TokenAccount>,
            pub token_program: Program<Token>,
            pub system_program: Program<System>,
        }
        pub struct TakeBumps {
            pub escrow_state: u8,
        }
        #[automatically_derived]
        #[doc(hidden)]
        unsafe impl ::core::clone::TrivialClone for TakeBumps {}
        #[automatically_derived]
        impl ::core::clone::Clone for TakeBumps {
            #[inline]
            fn clone(&self) -> TakeBumps {
                let _: ::core::clone::AssertParamIsClone<u8>;
                *self
            }
        }
        #[automatically_derived]
        impl ::core::marker::Copy for TakeBumps {}
        #[automatically_derived]
        impl ::core::default::Default for TakeBumps {
            #[inline]
            fn default() -> TakeBumps {
                TakeBumps {
                    escrow_state: ::core::default::Default::default(),
                }
            }
        }
        impl naclac_lang::prelude::EventCpiAccounts for Take {
            const NEEDS_EVENT_CPI: bool = false;
        }
        impl Take {
            pub fn to_account_metas(
                &self,
            ) -> [naclac_lang::prelude::AccountMeta; 11usize] {
                [
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.taker).address(),
                        is_signer: <Signer as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.maker).address(),
                        is_signer: <AccountInfo as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.mint_a).address(),
                        is_signer: <Account<
                            Mint,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: false,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.mint_b).address(),
                        is_signer: <Account<
                            Mint,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: false,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.escrow_state).address(),
                        is_signer: <Account<
                            EscrowState,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.vault_token_account).address(),
                        is_signer: <Account<
                            TokenAccount,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.taker_token_account_a).address(),
                        is_signer: <Account<
                            TokenAccount,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.taker_token_account_b).address(),
                        is_signer: <Account<
                            TokenAccount,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.maker_token_account_b).address(),
                        is_signer: <Account<
                            TokenAccount,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.token_program).address(),
                        is_signer: <Program<
                            Token,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: false,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.system_program).address(),
                        is_signer: <Program<
                            System,
                        > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                        is_writable: false,
                    },
                ]
            }
        }
        impl Take {
            pub fn to_account_infos(
                &self,
            ) -> [Option<naclac_lang::prelude::AccountInfo>; 11usize] {
                [
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(&self.taker),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(&self.maker),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.mint_a,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.mint_b,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.escrow_state,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.vault_token_account,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.taker_token_account_a,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.taker_token_account_b,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.maker_token_account_b,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.token_program,
                        ),
                    ),
                    Some(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &self.system_program,
                        ),
                    ),
                ]
            }
        }
        impl naclac_lang::prelude::Bumps for Take {
            type BumpsStruct = TakeBumps;
        }
        impl Take {
            pub const MUT_MASK: [u64; 4] = {
                let mut __mask = [0u64; 4];
                __mask = naclac_lang::prelude::mut_mask_set_bit(__mask, 0usize);
                __mask = naclac_lang::prelude::mut_mask_set_bit(__mask, 1usize);
                __mask = naclac_lang::prelude::mut_mask_set_bit(__mask, 4usize);
                __mask = naclac_lang::prelude::mut_mask_set_bit(__mask, 5usize);
                __mask = naclac_lang::prelude::mut_mask_set_bit(__mask, 6usize);
                __mask = naclac_lang::prelude::mut_mask_set_bit(__mask, 7usize);
                __mask = naclac_lang::prelude::mut_mask_set_bit(__mask, 8usize);
                __mask
            };
            /// Loads and validates the accounts provided to the instruction.
            ///
            /// Walks the serialized accounts at `__input` itself: each declared
            /// field's signer/writable/executable requirement is checked as the
            /// account is walked, then any trailing accounts are walked into the
            /// remaining-accounts slice. Rejects more than 64 account entries
            /// (the capacity of `__lookup`) and fewer than this struct declares.
            ///
            /// # Safety
            ///
            /// `__input` must be the runtime-provided entrypoint input buffer and
            /// `__num_accounts` the account count stored at its start.
            pub unsafe fn load_and_validate<const __NACLAC_IX_DISC_LEN: usize>(
                program_id: &'static naclac_lang::prelude::Address,
                __input: *mut u8,
                __num_accounts: usize,
                __lookup: &'static mut [core::mem::MaybeUninit<
                    naclac_lang::prelude::AccountView,
                >; 64],
                instruction_data: &[u8],
            ) -> Result<
                (Self, &'static [naclac_lang::prelude::AccountType], TakeBumps),
                naclac_lang::prelude::ProgramError,
            > {
                if __num_accounts.wrapping_sub(11usize) > 53usize {
                    return Err(
                        naclac_lang::prelude::account_count_error(__num_accounts),
                    );
                }
                let __lookup_ptr = __lookup.as_mut_ptr()
                    as *mut naclac_lang::prelude::AccountView;
                let mut __cursor = unsafe {
                    naclac_lang::prelude::AccountCursor::new(
                        __input,
                        __lookup_ptr,
                        program_id,
                    )
                };
                const __DECLARED_CHECKS: [(u32, u32, bool); 11usize] = [
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Signer as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            true,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <AccountInfo as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            true,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Account<
                                Mint,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            false,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Account<
                                Mint,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            false,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Account<
                                EscrowState,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            true,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Account<
                                TokenAccount,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            true,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Account<
                                TokenAccount,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            true,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Account<
                                TokenAccount,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            true,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Account<
                                TokenAccount,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            true,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Program<
                                Token,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            false,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                    {
                        let (__expected, __mask) = naclac_lang::prelude::account_header_check(
                            <Program<
                                System,
                            > as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                            false,
                            false,
                        );
                        (__expected, __mask, false)
                    },
                ];
                let mut __declared = [core::mem::MaybeUninit::<
                    naclac_lang::prelude::AccountView,
                >::uninit(); 11usize];
                if !unsafe {
                    __cursor
                        .try_declared_fast(
                            __DECLARED_CHECKS[0].0,
                            __DECLARED_CHECKS[0].1,
                        )
                        .is_some_and(|__view| {
                            __declared[0].write(__view);
                            true
                        })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[1].0,
                                __DECLARED_CHECKS[1].1,
                            )
                            .is_some_and(|__view| {
                                __declared[1].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[2].0,
                                __DECLARED_CHECKS[2].1,
                            )
                            .is_some_and(|__view| {
                                __declared[2].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[3].0,
                                __DECLARED_CHECKS[3].1,
                            )
                            .is_some_and(|__view| {
                                __declared[3].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[4].0,
                                __DECLARED_CHECKS[4].1,
                            )
                            .is_some_and(|__view| {
                                __declared[4].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[5].0,
                                __DECLARED_CHECKS[5].1,
                            )
                            .is_some_and(|__view| {
                                __declared[5].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[6].0,
                                __DECLARED_CHECKS[6].1,
                            )
                            .is_some_and(|__view| {
                                __declared[6].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[7].0,
                                __DECLARED_CHECKS[7].1,
                            )
                            .is_some_and(|__view| {
                                __declared[7].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[8].0,
                                __DECLARED_CHECKS[8].1,
                            )
                            .is_some_and(|__view| {
                                __declared[8].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[9].0,
                                __DECLARED_CHECKS[9].1,
                            )
                            .is_some_and(|__view| {
                                __declared[9].write(__view);
                                true
                            })
                        && __cursor
                            .try_declared_fast(
                                __DECLARED_CHECKS[10].0,
                                __DECLARED_CHECKS[10].1,
                            )
                            .is_some_and(|__view| {
                                __declared[10].write(__view);
                                true
                            })
                } || __num_accounts > 11usize
                {
                    unsafe {
                        __cursor
                            .finish_walk_slow(
                                __input,
                                &__DECLARED_CHECKS,
                                __num_accounts,
                                &Self::MUT_MASK,
                            )
                    }?;
                    unsafe {
                        __declared[0].write(*__lookup_ptr.add(0));
                        __declared[1].write(*__lookup_ptr.add(1));
                        __declared[2].write(*__lookup_ptr.add(2));
                        __declared[3].write(*__lookup_ptr.add(3));
                        __declared[4].write(*__lookup_ptr.add(4));
                        __declared[5].write(*__lookup_ptr.add(5));
                        __declared[6].write(*__lookup_ptr.add(6));
                        __declared[7].write(*__lookup_ptr.add(7));
                        __declared[8].write(*__lookup_ptr.add(8));
                        __declared[9].write(*__lookup_ptr.add(9));
                        __declared[10].write(*__lookup_ptr.add(10));
                    }
                }
                let __declared_infos = unsafe {
                    core::mem::transmute::<
                        [core::mem::MaybeUninit<
                            naclac_lang::prelude::AccountView,
                        >; 11usize],
                        [naclac_lang::prelude::AccountType; 11usize],
                    >(__declared)
                };
                let __views = &__declared_infos;
                let accounts = __views;
                let mut __ix_offset: usize = __NACLAC_IX_DISC_LEN;
                let seed = <u64 as naclac_lang::prelude::NaclacArgs>::naclac_deserialize(
                    instruction_data,
                    &mut __ix_offset,
                )?;
                let _ = &seed;
                let info = &__views[0usize];
                if <Signer as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(0usize),
                        );
                    }
                }
                let mut taker = {
                    const _: () = if !<Signer as naclac_lang::prelude::NaclacAccount>::IS_SIGNER {
                        {
                            ::core::panicking::panic_fmt(
                                format_args!(
                                    "a signer slot built without a signer check must be a type with IS_SIGNER",
                                ),
                            );
                        }
                    };
                    let __loaded = unsafe { <Signer>::from_verified_signer(info) };
                    __loaded
                };
                let info = &__views[1usize];
                if <AccountInfo as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(1usize),
                        );
                    }
                }
                let mut maker = <AccountInfo as naclac_lang::prelude::NaclacAccount>::try_from_mut(
                    info,
                    1usize,
                )?;
                let info = &__views[2usize];
                if <Account<
                    Mint,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(2usize),
                        );
                    }
                }
                let mut mint_a = <Account<
                    Mint,
                > as naclac_lang::prelude::NaclacAccount>::try_from(info, 2usize)?;
                let info = &__views[3usize];
                if <Account<
                    Mint,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(3usize),
                        );
                    }
                }
                let mut mint_b = <Account<
                    Mint,
                > as naclac_lang::prelude::NaclacAccount>::try_from(info, 3usize)?;
                let info = &__views[4usize];
                if <Account<
                    EscrowState,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(4usize),
                        );
                    }
                }
                let mut escrow_state = <Account<
                    EscrowState,
                > as naclac_lang::prelude::NaclacAccount>::try_from_mut(info, 4usize)?;
                let __seed_escrow_state_0 = &SEED_ESCROW;
                let __seed_escrow_state_1 = naclac_lang::prelude::ToAddress::address(
                    &(maker.address()),
                );
                let __seed_escrow_state_2 = &&seed.to_le_bytes();
                let expected_bump = (escrow_state.bump);
                let __pda_program: &naclac_lang::prelude::Address = program_id;
                let __seeds: &[&[u8]] = &[
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_escrow_state_0,
                    ),
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_escrow_state_1,
                    ),
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_escrow_state_2,
                    ),
                ];
                let expected_pda = naclac_lang::prelude::derive_program_address(
                    __seeds,
                    expected_bump,
                    __pda_program,
                );
                {
                    let __actual_address: &naclac_lang::prelude::Address = naclac_lang::prelude::Address::from_address_ref(
                        info.view.address(),
                    );
                    if *__actual_address != expected_pda {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintSeeds
                                .err(4usize),
                        );
                    }
                }
                if expected_bump != (escrow_state.bump) {
                    return Err(
                        naclac_lang::prelude::NaclacError::ConstraintSeeds.err(4usize),
                    );
                }
                let __bump_escrow_state = expected_bump;
                let info = &__views[5usize];
                if <Account<
                    TokenAccount,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(5usize),
                        );
                    }
                }
                let mut vault_token_account = <Account<
                    TokenAccount,
                > as naclac_lang::prelude::NaclacAccount>::try_from_mut(info, 5usize)?;
                {
                    const _: () = naclac_lang::token_constraints::token::RULES
                        .check(
                            &["mint", "authority"],
                            naclac_lang::prelude::ConstraintPhase::Check,
                        );
                    <naclac_lang::token_constraints::token::Constraint as naclac_lang::prelude::AccountConstraint<
                        _,
                    >>::check(
                            &vault_token_account,
                            &<naclac_lang::token_constraints::token::Args as ::core::default::Default>::default()
                                .mint(
                                    naclac_lang::prelude::ConstraintValue::into_value(
                                        &accounts[2usize],
                                    ),
                                )
                                .authority(
                                    naclac_lang::prelude::ConstraintValue::into_value(
                                        &accounts[4usize],
                                    ),
                                ),
                        )
                        .map_err(|e| e.err(5usize))?;
                }
                let info = &__views[6usize];
                if <Account<
                    TokenAccount,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(6usize),
                        );
                    }
                }
                let mut taker_token_account_a = <Account<
                    TokenAccount,
                > as naclac_lang::prelude::NaclacAccount>::try_from_mut(info, 6usize)?;
                {
                    const _: () = naclac_lang::token_constraints::token::RULES
                        .check(
                            &["mint", "authority"],
                            naclac_lang::prelude::ConstraintPhase::Check,
                        );
                    <naclac_lang::token_constraints::token::Constraint as naclac_lang::prelude::AccountConstraint<
                        _,
                    >>::check(
                            &taker_token_account_a,
                            &<naclac_lang::token_constraints::token::Args as ::core::default::Default>::default()
                                .mint(
                                    naclac_lang::prelude::ConstraintValue::into_value(
                                        &accounts[2usize],
                                    ),
                                )
                                .authority(
                                    naclac_lang::prelude::ConstraintValue::into_value(&taker),
                                ),
                        )
                        .map_err(|e| e.err(6usize))?;
                }
                let info = &__views[7usize];
                if <Account<
                    TokenAccount,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(7usize),
                        );
                    }
                }
                let mut taker_token_account_b = <Account<
                    TokenAccount,
                > as naclac_lang::prelude::NaclacAccount>::try_from_mut(info, 7usize)?;
                {
                    const _: () = naclac_lang::token_constraints::token::RULES
                        .check(
                            &["mint", "authority"],
                            naclac_lang::prelude::ConstraintPhase::Check,
                        );
                    <naclac_lang::token_constraints::token::Constraint as naclac_lang::prelude::AccountConstraint<
                        _,
                    >>::check(
                            &taker_token_account_b,
                            &<naclac_lang::token_constraints::token::Args as ::core::default::Default>::default()
                                .mint(
                                    naclac_lang::prelude::ConstraintValue::into_value(
                                        &accounts[3usize],
                                    ),
                                )
                                .authority(
                                    naclac_lang::prelude::ConstraintValue::into_value(&taker),
                                ),
                        )
                        .map_err(|e| e.err(7usize))?;
                }
                let info = &__views[8usize];
                if <Account<
                    TokenAccount,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(8usize),
                        );
                    }
                }
                let mut maker_token_account_b = <Account<
                    TokenAccount,
                > as naclac_lang::prelude::NaclacAccount>::try_from_mut(info, 8usize)?;
                {
                    const _: () = naclac_lang::token_constraints::token::RULES
                        .check(
                            &["mint", "authority"],
                            naclac_lang::prelude::ConstraintPhase::Check,
                        );
                    <naclac_lang::token_constraints::token::Constraint as naclac_lang::prelude::AccountConstraint<
                        _,
                    >>::check(
                            &maker_token_account_b,
                            &<naclac_lang::token_constraints::token::Args as ::core::default::Default>::default()
                                .mint(
                                    naclac_lang::prelude::ConstraintValue::into_value(
                                        &accounts[3usize],
                                    ),
                                )
                                .authority(
                                    naclac_lang::prelude::ConstraintValue::into_value(
                                        &accounts[1usize],
                                    ),
                                ),
                        )
                        .map_err(|e| e.err(8usize))?;
                }
                let info = &__views[9usize];
                const _: () = if !<Program<
                    Token,
                > as naclac_lang::prelude::NaclacAccount>::CHECKS_PROGRAM_ID {
                    {
                        ::core::panicking::panic_fmt(
                            format_args!(
                                "no address check is emitted for this field, so its type must check the program id",
                            ),
                        );
                    }
                };
                if <Program<
                    Token,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(9usize),
                        );
                    }
                }
                let mut token_program = <Program<
                    Token,
                > as naclac_lang::prelude::NaclacAccount>::try_from(info, 9usize)?;
                let info = &__views[10usize];
                const _: () = if !<Program<
                    System,
                > as naclac_lang::prelude::NaclacAccount>::CHECKS_PROGRAM_ID {
                    {
                        ::core::panicking::panic_fmt(
                            format_args!(
                                "no address check is emitted for this field, so its type must check the program id",
                            ),
                        );
                    }
                };
                if <Program<
                    System,
                > as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                    if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(10usize),
                        );
                    }
                }
                let mut system_program = <Program<
                    System,
                > as naclac_lang::prelude::NaclacAccount>::try_from(info, 10usize)?;
                let __remaining: &'static [naclac_lang::prelude::AccountType] = unsafe {
                    core::slice::from_raw_parts(
                        (__lookup_ptr as *const naclac_lang::prelude::AccountType)
                            .add(11usize),
                        __num_accounts.wrapping_sub(11usize),
                    )
                };
                let mut __me = Self {
                    taker,
                    maker,
                    mint_a,
                    mint_b,
                    escrow_state,
                    vault_token_account,
                    taker_token_account_a,
                    taker_token_account_b,
                    maker_token_account_b,
                    token_program,
                    system_program,
                };
                let __val = __me.escrow_state.mint_a;
                let __target_key = (__me.mint_a).address();
                if __val != __target_key {
                    return Err(EscrowError::InvalidMint.into());
                }
                let __val = __me.escrow_state.mint_b;
                let __target_key = (__me.mint_b).address();
                if __val != __target_key {
                    return Err(EscrowError::InvalidMint.into());
                }
                Ok((
                    __me,
                    __remaining,
                    TakeBumps {
                        escrow_state: __bump_escrow_state,
                    },
                ))
            }
            /// Persists mutations and handles cleanup.
            ///
            /// For zero-copy accounts this is a no-op (`Ok(())`). Marked `#[inline(always)]`
            /// so the compiler eliminates the call and the conditional branch in the dispatcher
            /// when the body is empty.
            #[inline(always)]
            pub fn teardown(
                &mut self,
                program_id: &naclac_lang::prelude::Address,
                bumps: &TakeBumps,
            ) -> naclac_lang::prelude::Result {
                let __target = &self.escrow_state;
                if naclac_lang::prelude::ToAddress::address(__target)
                    == naclac_lang::prelude::ToAddress::address(&self.maker)
                {
                    return Err(
                        naclac_lang::prelude::NaclacError::ConstraintClose.err(4usize),
                    );
                }
                {
                    let target_info = naclac_lang::prelude::ToAccountInfo::to_account_info(
                        __target,
                    );
                    let dest_info = naclac_lang::prelude::ToAccountInfo::to_account_info(
                        &self.maker,
                    );
                    if target_info.owner() != *program_id {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(4usize),
                        );
                    }
                    let target_lamports = target_info.lamports();
                    dest_info.add_lamports(target_lamports)?;
                    target_info.sub_lamports(target_lamports)?;
                    let mut target_view = target_info.view;
                    unsafe {
                        target_view
                            .assign(
                                naclac_lang::prelude::SYSTEM_PROGRAM_ID.as_address(),
                            );
                        core::ptr::write_bytes(
                            target_view.data_ptr() as *mut u8,
                            0,
                            target_view.data_len(),
                        );
                    }
                }
                Ok(())
            }
        }
        pub fn take(ctx: Context<Take>, seed: u64) -> Result {
            let decimals_b = ctx.accounts.mint_b.decimals();
            ctx.accounts
                .token_program
                .transfer_checked(
                    TransferCheckedAccounts {
                        from: &mut ctx.accounts.taker_token_account_b,
                        mint: &ctx.accounts.mint_b,
                        to: &mut ctx.accounts.maker_token_account_b,
                        authority: &ctx.accounts.taker,
                    },
                    ctx.accounts.escrow_state.amount_b,
                    decimals_b,
                )?;
            let maker_addr = ctx.accounts.maker.address();
            let seed_bytes = seed.to_le_bytes();
            let seeds: &[&[u8]] = &[
                SEED_ESCROW,
                maker_addr.as_ref(),
                &seed_bytes,
                &[ctx.accounts.escrow_state.bump],
            ];
            let signer_seeds: &[&[&[u8]]] = &[seeds];
            let decimals_a = ctx.accounts.mint_a.decimals();
            ctx.accounts
                .token_program
                .transfer_checked_signed(
                    TransferCheckedAccounts {
                        from: &mut ctx.accounts.vault_token_account,
                        mint: &ctx.accounts.mint_a,
                        to: &mut ctx.accounts.taker_token_account_a,
                        authority: &ctx.accounts.escrow_state,
                    },
                    ctx.accounts.escrow_state.amount_a,
                    decimals_a,
                    signer_seeds,
                )?;
            ctx.accounts
                .token_program
                .close_account_signed(
                    CloseAccountAccounts {
                        account: &mut ctx.accounts.vault_token_account,
                        destination: &mut ctx.accounts.maker,
                        authority: &ctx.accounts.escrow_state,
                    },
                    signer_seeds,
                )?;
            {
                let mut __event = <EscrowExchanged as core::default::Default>::default();
                __event.taker = (ctx.accounts.taker.address()).into();
                __event.amount_a = (ctx.accounts.escrow_state.amount_a).into();
                __event.amount_b = (ctx.accounts.escrow_state.amount_b).into();
                __event.emit();
                __event
            };
            Ok(())
        }
    }
    pub use cancel::*;
    pub use make::*;
    pub use take::*;
}
pub mod state {
    pub mod escrow_state {
        use naclac_lang::prelude::*;
        #[repr(C, packed)]
        pub struct EscrowState {
            pub maker: Address,
            pub mint_a: Address,
            pub mint_b: Address,
            pub amount_a: u64,
            pub amount_b: u64,
            pub bump: u8,
        }
        const _: () = {
            if !(::core::mem::size_of::<EscrowState>()
                == (::core::mem::size_of::<Address>() + ::core::mem::size_of::<Address>()
                    + ::core::mem::size_of::<Address>() + ::core::mem::size_of::<u64>()
                    + ::core::mem::size_of::<u64>() + ::core::mem::size_of::<u8>()))
            {
                {
                    ::core::panicking::panic_fmt(
                        format_args!(
                            "defined_type/#[account]: `EscrowState` has a different size than its fields add up to; a zero-copy struct is `#[repr(C, packed)]` and must have no padding",
                        ),
                    );
                }
            }
            if !(::core::mem::align_of::<EscrowState>() == 1) {
                {
                    ::core::panicking::panic_fmt(
                        format_args!(
                            "defined_type/#[account]: `EscrowState` must have alignment 1; a zero-copy struct is `#[repr(C, packed)]`",
                        ),
                    );
                }
            }
        };
        const _: fn() = || {
            fn assert_impl<T: naclac_lang::prelude::ZcField>() {}
            assert_impl::<Address>();
            assert_impl::<Address>();
            assert_impl::<Address>();
            assert_impl::<u64>();
            assert_impl::<u64>();
            assert_impl::<u8>();
        };
        unsafe impl naclac_lang::prelude::bytemuck::Pod for EscrowState {}
        unsafe impl naclac_lang::prelude::bytemuck::Zeroable for EscrowState {}
        impl core::default::Default for EscrowState {
            fn default() -> Self {
                naclac_lang::prelude::bytemuck::Zeroable::zeroed()
            }
        }
        impl core::clone::Clone for EscrowState {
            #[inline(always)]
            fn clone(&self) -> Self {
                *self
            }
        }
        impl core::marker::Copy for EscrowState {}
        impl EscrowState {
            pub const DISCRIMINATOR: &'static [u8] = &[
                19u8, 90u8, 148u8, 111u8, 55u8, 130u8, 229u8, 108u8,
            ];
            pub const SPACE: usize = core::mem::size_of::<Self>()
                + Self::DISCRIMINATOR.len();
            #[inline(always)]
            pub fn load(data: &[u8]) -> naclac_lang::prelude::Result<&Self> {
                if data.len() < Self::SPACE {
                    return Err(
                        naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(0),
                    );
                }
                if &data[..Self::DISCRIMINATOR.len()] != Self::DISCRIMINATOR {
                    return Err(
                        naclac_lang::prelude::NaclacError::InvalidAccountDiscriminator
                            .err(0),
                    );
                }
                Ok(
                    naclac_lang::prelude::bytemuck::from_bytes(
                        &data[Self::DISCRIMINATOR.len()..Self::SPACE],
                    ),
                )
            }
            #[inline(always)]
            pub fn load_mut(data: &mut [u8]) -> naclac_lang::prelude::Result<&mut Self> {
                if data.len() < Self::SPACE {
                    return Err(
                        naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(0),
                    );
                }
                if &data[..Self::DISCRIMINATOR.len()] != Self::DISCRIMINATOR {
                    return Err(
                        naclac_lang::prelude::NaclacError::InvalidAccountDiscriminator
                            .err(0),
                    );
                }
                Ok(
                    naclac_lang::prelude::bytemuck::from_bytes_mut(
                        &mut data[Self::DISCRIMINATOR.len()..Self::SPACE],
                    ),
                )
            }
        }
        impl naclac_lang::prelude::NaclacZeroCopy for EscrowState {}
        impl naclac_lang::prelude::Discriminator for EscrowState {
            const DISCRIMINATOR: &'static [u8] = &[
                19u8, 90u8, 148u8, 111u8, 55u8, 130u8, 229u8, 108u8,
            ];
        }
    }
    pub use escrow_state::*;
}
use instructions::*;
const _: bool = naclac_lang::__NACLAC_LANG_HAS_PINOCCHIO_RUNTIME;
const _: bool = naclac_lang::__NACLAC_LANG_HAS_BYTEMUCK_LAYOUT;
#[doc(hidden)]
pub const __NACLAC_EVENT_DISPATCH_TAG: &[u8] = &[
    34u8, 131u8, 189u8, 8u8, 189u8, 141u8, 224u8, 48u8,
];
#[doc(hidden)]
pub const __NACLAC_EVENT_CPI_IN_USE: bool = escrow::__NACLAC_NEEDS_EVENT_CPI;
#[no_mangle]
pub unsafe extern "C" fn entrypoint(input: *mut u8, ix_data_ptr: *const u8) -> u64 {
    let num_accounts = unsafe { *(input as *const u64) } as usize;
    let len = unsafe { *(ix_data_ptr.sub(8) as *const u64) } as usize;
    let (
        instruction_data,
        program_id,
    ): (&'static [u8], &'static naclac_lang::prelude::Address) = unsafe {
        (
            core::slice::from_raw_parts(ix_data_ptr, len),
            &*(ix_data_ptr.add(len) as *const naclac_lang::prelude::Address),
        )
    };
    __naclac_dispatch_to_u64(program_id, input, num_accounts, instruction_data)
}
/// Same dispatch as `process_instruction_inner`, but each arm converts
/// its own result, so a successful arm returns `SUCCESS` directly.
#[inline(always)]
fn __naclac_dispatch_to_u64(
    program_id: &'static naclac_lang::prelude::Address,
    input: *mut u8,
    num_accounts: usize,
    instruction_data: &'static [u8],
) -> u64 {
    if instruction_data.len() < 8usize {
        return naclac_lang::prelude::program_result_to_u64(
            Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)),
        );
    }
    let instruction_discriminator = u64::from_le_bytes(unsafe {
        *(instruction_data.as_ptr() as *const [u8; 8])
    });
    let mut __lookup_storage = [core::mem::MaybeUninit::<
        naclac_lang::prelude::AccountView,
    >::uninit(); 64];
    let __lookup: &'static mut [core::mem::MaybeUninit<
        naclac_lang::prelude::AccountView,
    >; 64] = unsafe { &mut *(&mut __lookup_storage as *mut _) };
    match instruction_discriminator {
        14222551101251052426u64 => {
            naclac_lang::prelude::program_result_to_u64({
                {
                    escrow::__dispatch_make(
                        program_id,
                        input,
                        num_accounts,
                        __lookup,
                        instruction_data,
                    )
                }
            })
        }
        2875141570291884693u64 => {
            naclac_lang::prelude::program_result_to_u64({
                {
                    escrow::__dispatch_take(
                        program_id,
                        input,
                        num_accounts,
                        __lookup,
                        instruction_data,
                    )
                }
            })
        }
        13753127788127181800u64 => {
            naclac_lang::prelude::program_result_to_u64({
                {
                    escrow::__dispatch_cancel(
                        program_id,
                        input,
                        num_accounts,
                        __lookup,
                        instruction_data,
                    )
                }
            })
        }
        3521970751638700834u64 if escrow::__NACLAC_NEEDS_EVENT_CPI => {
            naclac_lang::prelude::program_result_to_u64({
                { __naclac_receive_event_cpi(input, num_accounts) }
            })
        }
        _ => {
            naclac_lang::prelude::program_result_to_u64(
                Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)),
            )
        }
    }
}
/// Handles an `emit_cpi!` self-invocation: succeeds only when the
/// first account is the program's event authority PDA and signed.
#[inline(always)]
fn __naclac_receive_event_cpi(
    input: *mut u8,
    num_accounts: usize,
) -> naclac_lang::prelude::ProgramResult {
    const _: &[u8] = b"#![no_std]\nuse naclac_lang::prelude::*;\n\ndeclare_id!(\"BUnzdon444sj9eHgaCNgnNk3ckdKc5xSgyax946Uuvau\");\n\npub mod constants;\npub mod errors;\npub mod events;\npub mod instructions;\npub mod state;\n\nuse instructions::*;\n\n#[program]\npub mod escrow {\n    pub fn make(ctx: Context<Make>, seed: u64, amount_a: u64, amount_b: u64) -> Result {\n        make::make(ctx, seed, amount_a, amount_b)\n    }\n\n    pub fn take(ctx: Context<Take>, seed: u64) -> Result {\n        take::take(ctx, seed)\n    }\n\n    pub fn cancel(ctx: Context<Cancel>, seed: u64) -> Result {\n        cancel::cancel(ctx, seed)\n    }\n}\n";
    {
        if num_accounts == 0 {
            return Err(naclac_lang::prelude::NaclacError::NotEnoughAccountKeys.err(0));
        }
        let __ea = unsafe {
            naclac_lang::prelude::AccountView::new_unchecked(
                input.add(8) as *mut naclac_lang::pinocchio::account::RuntimeAccount,
            )
        };
        if !__ea.is_signer() {
            return Err(naclac_lang::prelude::NaclacError::ConstraintSigner.err(0));
        }
        if naclac_lang::prelude::Address::from_address_ref(__ea.address())
            != &naclac_lang::prelude::Address::new_from_array([
                249u8, 182u8, 238u8, 83u8, 48u8, 86u8, 180u8, 201u8, 234u8, 118u8, 160u8,
                59u8, 128u8, 55u8, 66u8, 229u8, 10u8, 245u8, 189u8, 151u8, 222u8, 207u8,
                78u8, 15u8, 239u8, 40u8, 137u8, 35u8, 152u8, 161u8, 229u8, 27u8,
            ])
        {
            return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(0));
        }
        Ok(())
    }
}
/// A default allocator for when the program is compiled on a target different
/// than `"solana"`.
///
/// This links the `std` library, which will set up a default global allocator.
mod __private_alloc {
    extern crate std as __std;
}
/// Unified Entrypoint for Pinocchio — parses raw runtime inputs.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers (`input` and `ix_data_ptr`)
/// supplied directly by the Solana runtime BPF loader. The caller must ensure these
/// pointers point to valid, aligned, and initialized memory.
pub unsafe fn process_instruction(
    input: *mut u8,
    ix_data_ptr: *const u8,
) -> naclac_lang::prelude::ProgramResult {
    let num_accounts = unsafe { *(input as *const u64) } as usize;
    let instruction_data: &'static [u8] = unsafe {
        let len = *(ix_data_ptr.sub(8) as *const u64) as usize;
        core::slice::from_raw_parts(ix_data_ptr, len)
    };
    let program_id: &'static naclac_lang::prelude::Address = unsafe {
        let len = *(ix_data_ptr.sub(8) as *const u64) as usize;
        &*(ix_data_ptr.add(len) as *const naclac_lang::prelude::Address)
    };
    process_instruction_inner(program_id, input, num_accounts, instruction_data)
}
#[inline(always)]
fn process_instruction_inner(
    program_id: &'static naclac_lang::prelude::Address,
    input: *mut u8,
    num_accounts: usize,
    instruction_data: &'static [u8],
) -> naclac_lang::prelude::ProgramResult {
    if instruction_data.len() < 8usize {
        return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
    }
    let instruction_discriminator = u64::from_le_bytes(unsafe {
        *(instruction_data.as_ptr() as *const [u8; 8])
    });
    let mut __lookup_storage = [core::mem::MaybeUninit::<
        naclac_lang::prelude::AccountView,
    >::uninit(); 64];
    let __lookup: &'static mut [core::mem::MaybeUninit<
        naclac_lang::prelude::AccountView,
    >; 64] = unsafe { &mut *(&mut __lookup_storage as *mut _) };
    match instruction_discriminator {
        14222551101251052426u64 => {
            escrow::__dispatch_make(
                program_id,
                input,
                num_accounts,
                __lookup,
                instruction_data,
            )
        }
        2875141570291884693u64 => {
            escrow::__dispatch_take(
                program_id,
                input,
                num_accounts,
                __lookup,
                instruction_data,
            )
        }
        13753127788127181800u64 => {
            escrow::__dispatch_cancel(
                program_id,
                input,
                num_accounts,
                __lookup,
                instruction_data,
            )
        }
        3521970751638700834u64 if escrow::__NACLAC_NEEDS_EVENT_CPI => {
            __naclac_receive_event_cpi(input, num_accounts)
        }
        _ => Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)),
    }
}
pub mod escrow {
    use super::*;
    pub(crate) fn __dispatch_make(
        program_id: &'static naclac_lang::prelude::Address,
        __input: *mut u8,
        __num_accounts: usize,
        __lookup: &'static mut [core::mem::MaybeUninit<
            naclac_lang::prelude::AccountView,
        >; 64],
        instruction_data: &'static [u8],
    ) -> naclac_lang::prelude::ProgramResult {
        let mut __ix_offset: usize = 8usize;
        let seed = <u64 as naclac_lang::prelude::NaclacArgs>::naclac_deserialize(
            instruction_data,
            &mut __ix_offset,
        )?;
        let amount_a = <u64 as naclac_lang::prelude::NaclacArgs>::naclac_deserialize(
            instruction_data,
            &mut __ix_offset,
        )?;
        let amount_b = <u64 as naclac_lang::prelude::NaclacArgs>::naclac_deserialize(
            instruction_data,
            &mut __ix_offset,
        )?;
        let (mut accounts, __rem_slice, bumps) = unsafe {
            Make::load_and_validate::<
                8usize,
            >(program_id, __input, __num_accounts, __lookup, instruction_data)
        }?;
        let ctx = unsafe {
            naclac_lang::prelude::Context::new(
                program_id,
                &mut accounts,
                __rem_slice,
                bumps,
            )
        };
        let __result = escrow::make(ctx, seed, amount_a, amount_b);
        if __result.is_ok() {
            accounts.teardown(program_id, &bumps)?;
        }
        __result.map(|_| ())
    }
    pub fn make(ctx: Context<Make>, seed: u64, amount_a: u64, amount_b: u64) -> Result {
        make::make(ctx, seed, amount_a, amount_b)
    }
    pub(crate) fn __dispatch_take(
        program_id: &'static naclac_lang::prelude::Address,
        __input: *mut u8,
        __num_accounts: usize,
        __lookup: &'static mut [core::mem::MaybeUninit<
            naclac_lang::prelude::AccountView,
        >; 64],
        instruction_data: &'static [u8],
    ) -> naclac_lang::prelude::ProgramResult {
        let mut __ix_offset: usize = 8usize;
        let seed = <u64 as naclac_lang::prelude::NaclacArgs>::naclac_deserialize(
            instruction_data,
            &mut __ix_offset,
        )?;
        let (mut accounts, __rem_slice, bumps) = unsafe {
            Take::load_and_validate::<
                8usize,
            >(program_id, __input, __num_accounts, __lookup, instruction_data)
        }?;
        let ctx = unsafe {
            naclac_lang::prelude::Context::new(
                program_id,
                &mut accounts,
                __rem_slice,
                bumps,
            )
        };
        let __result = escrow::take(ctx, seed);
        if __result.is_ok() {
            accounts.teardown(program_id, &bumps)?;
        }
        __result.map(|_| ())
    }
    pub fn take(ctx: Context<Take>, seed: u64) -> Result {
        take::take(ctx, seed)
    }
    pub(crate) fn __dispatch_cancel(
        program_id: &'static naclac_lang::prelude::Address,
        __input: *mut u8,
        __num_accounts: usize,
        __lookup: &'static mut [core::mem::MaybeUninit<
            naclac_lang::prelude::AccountView,
        >; 64],
        instruction_data: &'static [u8],
    ) -> naclac_lang::prelude::ProgramResult {
        let mut __ix_offset: usize = 8usize;
        let seed = <u64 as naclac_lang::prelude::NaclacArgs>::naclac_deserialize(
            instruction_data,
            &mut __ix_offset,
        )?;
        let (mut accounts, __rem_slice, bumps) = unsafe {
            Cancel::load_and_validate::<
                8usize,
            >(program_id, __input, __num_accounts, __lookup, instruction_data)
        }?;
        let ctx = unsafe {
            naclac_lang::prelude::Context::new(
                program_id,
                &mut accounts,
                __rem_slice,
                bumps,
            )
        };
        let __result = escrow::cancel(ctx, seed);
        if __result.is_ok() {
            accounts.teardown(program_id, &bumps)?;
        }
        __result.map(|_| ())
    }
    pub fn cancel(ctx: Context<Cancel>, seed: u64) -> Result {
        cancel::cancel(ctx, seed)
    }
    pub(super) const __NACLAC_NEEDS_EVENT_CPI: bool = false
        || <Make as naclac_lang::prelude::EventCpiAccounts>::NEEDS_EVENT_CPI
        || <Take as naclac_lang::prelude::EventCpiAccounts>::NEEDS_EVENT_CPI
        || <Cancel as naclac_lang::prelude::EventCpiAccounts>::NEEDS_EVENT_CPI;
    impl naclac_lang::prelude::EventCpiTag for Make {
        const EVENT_DISPATCH_TAG: &'static [u8] = &[
            34u8, 131u8, 189u8, 8u8, 189u8, 141u8, 224u8, 48u8,
        ];
    }
    impl naclac_lang::prelude::EventCpiTag for Take {
        const EVENT_DISPATCH_TAG: &'static [u8] = &[
            34u8, 131u8, 189u8, 8u8, 189u8, 141u8, 224u8, 48u8,
        ];
    }
    impl naclac_lang::prelude::EventCpiTag for Cancel {
        const EVENT_DISPATCH_TAG: &'static [u8] = &[
            34u8, 131u8, 189u8, 8u8, 189u8, 141u8, 224u8, 48u8,
        ];
    }
}
