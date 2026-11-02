#![feature(prelude_import)]
extern crate std;
#[prelude_import]
use std::prelude::rust_2021::*;
use naclac_lang::prelude::*;
pub const ID: ::naclac_core::solana_address::Address = ::solana_address::Address::from_str_const(
    "G5gui4RgZH8tjr6M6Js2yJqTcG9tVukUz8p27BNonEgT",
);
const _: () = ::naclac_core::address_literal::assert_program_address(
    "G5gui4RgZH8tjr6M6Js2yJqTcG9tVukUz8p27BNonEgT",
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
    #[borsh(crate = "naclac_lang::prelude::borsh")]
    pub struct EscrowCreated {
        pub maker: Address,
        pub amount_a: u64,
        pub amount_b: u64,
    }
    #[automatically_derived]
    impl ::core::clone::Clone for EscrowCreated {
        #[inline]
        fn clone(&self) -> EscrowCreated {
            EscrowCreated {
                maker: ::core::clone::Clone::clone(&self.maker),
                amount_a: ::core::clone::Clone::clone(&self.amount_a),
                amount_b: ::core::clone::Clone::clone(&self.amount_b),
            }
        }
    }
    #[automatically_derived]
    impl naclac_lang::prelude::borsh::ser::BorshSerialize for EscrowCreated {
        fn serialize<__W: naclac_lang::prelude::borsh::io::Write>(
            &self,
            writer: &mut __W,
        ) -> ::core::result::Result<(), naclac_lang::prelude::borsh::io::Error> {
            naclac_lang::prelude::borsh::BorshSerialize::serialize(&self.maker, writer)?;
            naclac_lang::prelude::borsh::BorshSerialize::serialize(
                &self.amount_a,
                writer,
            )?;
            naclac_lang::prelude::borsh::BorshSerialize::serialize(
                &self.amount_b,
                writer,
            )?;
            Ok(())
        }
    }
    #[automatically_derived]
    impl ::core::default::Default for EscrowCreated {
        #[inline]
        fn default() -> EscrowCreated {
            EscrowCreated {
                maker: ::core::default::Default::default(),
                amount_a: ::core::default::Default::default(),
                amount_b: ::core::default::Default::default(),
            }
        }
    }
    impl EscrowCreated {
        pub fn emit(&self) {
            naclac_lang::event::emit_event(
                self,
                &[70u8, 127u8, 105u8, 102u8, 92u8, 97u8, 7u8, 173u8],
            );
        }
    }
    impl naclac_lang::prelude::EventCpiPayload for EscrowCreated {
        fn with_cpi_payload<R>(&self, tag: &[u8], send: impl FnOnce(&[u8]) -> R) -> R {
            let discriminator: &[u8] = &[
                70u8, 127u8, 105u8, 102u8, 92u8, 97u8, 7u8, 173u8,
            ];
            let mut __buf = naclac_lang::prelude::Vec::<
                u8,
            >::with_capacity(tag.len() + discriminator.len() + 64);
            __buf.extend_from_slice(tag);
            __buf.extend_from_slice(discriminator);
            naclac_lang::prelude::BorshSerialize::serialize(self, &mut __buf)
                .unwrap_or_else(|_| {
                    {
                        ::core::panicking::panic_fmt(
                            format_args!(
                                "event serialization is infallible for all currently-supported field types",
                            ),
                        );
                    }
                });
            send(&__buf)
        }
    }
    #[borsh(crate = "naclac_lang::prelude::borsh")]
    pub struct EscrowExchanged {
        pub taker: Address,
        pub amount_a: u64,
        pub amount_b: u64,
    }
    #[automatically_derived]
    impl ::core::clone::Clone for EscrowExchanged {
        #[inline]
        fn clone(&self) -> EscrowExchanged {
            EscrowExchanged {
                taker: ::core::clone::Clone::clone(&self.taker),
                amount_a: ::core::clone::Clone::clone(&self.amount_a),
                amount_b: ::core::clone::Clone::clone(&self.amount_b),
            }
        }
    }
    #[automatically_derived]
    impl naclac_lang::prelude::borsh::ser::BorshSerialize for EscrowExchanged {
        fn serialize<__W: naclac_lang::prelude::borsh::io::Write>(
            &self,
            writer: &mut __W,
        ) -> ::core::result::Result<(), naclac_lang::prelude::borsh::io::Error> {
            naclac_lang::prelude::borsh::BorshSerialize::serialize(&self.taker, writer)?;
            naclac_lang::prelude::borsh::BorshSerialize::serialize(
                &self.amount_a,
                writer,
            )?;
            naclac_lang::prelude::borsh::BorshSerialize::serialize(
                &self.amount_b,
                writer,
            )?;
            Ok(())
        }
    }
    #[automatically_derived]
    impl ::core::default::Default for EscrowExchanged {
        #[inline]
        fn default() -> EscrowExchanged {
            EscrowExchanged {
                taker: ::core::default::Default::default(),
                amount_a: ::core::default::Default::default(),
                amount_b: ::core::default::Default::default(),
            }
        }
    }
    impl EscrowExchanged {
        pub fn emit(&self) {
            naclac_lang::event::emit_event(
                self,
                &[227u8, 173u8, 205u8, 57u8, 1u8, 154u8, 129u8, 42u8],
            );
        }
    }
    impl naclac_lang::prelude::EventCpiPayload for EscrowExchanged {
        fn with_cpi_payload<R>(&self, tag: &[u8], send: impl FnOnce(&[u8]) -> R) -> R {
            let discriminator: &[u8] = &[
                227u8, 173u8, 205u8, 57u8, 1u8, 154u8, 129u8, 42u8,
            ];
            let mut __buf = naclac_lang::prelude::Vec::<
                u8,
            >::with_capacity(tag.len() + discriminator.len() + 64);
            __buf.extend_from_slice(tag);
            __buf.extend_from_slice(discriminator);
            naclac_lang::prelude::BorshSerialize::serialize(self, &mut __buf)
                .unwrap_or_else(|_| {
                    {
                        ::core::panicking::panic_fmt(
                            format_args!(
                                "event serialization is infallible for all currently-supported field types",
                            ),
                        );
                    }
                });
            send(&__buf)
        }
    }
    #[borsh(crate = "naclac_lang::prelude::borsh")]
    pub struct EscrowCancelled {
        pub maker: Address,
        pub amount_a: u64,
    }
    #[automatically_derived]
    impl ::core::clone::Clone for EscrowCancelled {
        #[inline]
        fn clone(&self) -> EscrowCancelled {
            EscrowCancelled {
                maker: ::core::clone::Clone::clone(&self.maker),
                amount_a: ::core::clone::Clone::clone(&self.amount_a),
            }
        }
    }
    #[automatically_derived]
    impl naclac_lang::prelude::borsh::ser::BorshSerialize for EscrowCancelled {
        fn serialize<__W: naclac_lang::prelude::borsh::io::Write>(
            &self,
            writer: &mut __W,
        ) -> ::core::result::Result<(), naclac_lang::prelude::borsh::io::Error> {
            naclac_lang::prelude::borsh::BorshSerialize::serialize(&self.maker, writer)?;
            naclac_lang::prelude::borsh::BorshSerialize::serialize(
                &self.amount_a,
                writer,
            )?;
            Ok(())
        }
    }
    #[automatically_derived]
    impl ::core::default::Default for EscrowCancelled {
        #[inline]
        fn default() -> EscrowCancelled {
            EscrowCancelled {
                maker: ::core::default::Default::default(),
                amount_a: ::core::default::Default::default(),
            }
        }
    }
    impl EscrowCancelled {
        pub fn emit(&self) {
            naclac_lang::event::emit_event(
                self,
                &[98u8, 241u8, 195u8, 122u8, 213u8, 0u8, 162u8, 161u8],
            );
        }
    }
    impl naclac_lang::prelude::EventCpiPayload for EscrowCancelled {
        fn with_cpi_payload<R>(&self, tag: &[u8], send: impl FnOnce(&[u8]) -> R) -> R {
            let discriminator: &[u8] = &[
                98u8, 241u8, 195u8, 122u8, 213u8, 0u8, 162u8, 161u8,
            ];
            let mut __buf = naclac_lang::prelude::Vec::<
                u8,
            >::with_capacity(tag.len() + discriminator.len() + 64);
            __buf.extend_from_slice(tag);
            __buf.extend_from_slice(discriminator);
            naclac_lang::prelude::BorshSerialize::serialize(self, &mut __buf)
                .unwrap_or_else(|_| {
                    {
                        ::core::panicking::panic_fmt(
                            format_args!(
                                "event serialization is infallible for all currently-supported field types",
                            ),
                        );
                    }
                });
            send(&__buf)
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
                naclac_lang::prelude::NaclacAccount::exit(&self.maker, program_id)?;
                naclac_lang::prelude::NaclacAccount::exit(
                    &self.escrow_state,
                    program_id,
                )?;
                naclac_lang::prelude::NaclacAccount::exit(
                    &self.vault_token_account,
                    program_id,
                )?;
                naclac_lang::prelude::NaclacAccount::exit(
                    &self.maker_token_account_a,
                    program_id,
                )?;
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
                    if target_info.owner != program_id {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(2usize),
                        );
                    }
                    let dest_starting_lamports = dest_info.lamports();
                    **dest_info.lamports.borrow_mut() = dest_starting_lamports
                        .checked_add(target_info.lamports())
                        .unwrap();
                    **target_info.lamports.borrow_mut() = 0;
                    target_info.assign(&naclac_lang::prelude::SYSTEM_PROGRAM_ID);
                    target_info.data.borrow_mut().fill(0);
                }
                Ok(())
            }
        }
        const _: () = if !(core::mem::size_of::<Account<Mint>>() <= 300) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: field `mint_a` is large for a single stack frame — wrap it as `Box<Account<Mint>>` to keep `load_and_validate` under the SBF 4KB stack limit",
                    ),
                );
            }
        };
        const _: () = if !(core::mem::size_of::<Account<EscrowState>>() <= 300) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: field `escrow_state` is large for a single stack frame — wrap it as `Box<Account<EscrowState>>` to keep `load_and_validate` under the SBF 4KB stack limit",
                    ),
                );
            }
        };
        const _: () = if !(core::mem::size_of::<Account<TokenAccount>>() <= 300) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: field `vault_token_account` is large for a single stack frame — wrap it as `Box<Account<TokenAccount>>` to keep `load_and_validate` under the SBF 4KB stack limit",
                    ),
                );
            }
        };
        const _: () = if !(core::mem::size_of::<Account<TokenAccount>>() <= 300) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: field `maker_token_account_a` is large for a single stack frame — wrap it as `Box<Account<TokenAccount>>` to keep `load_and_validate` under the SBF 4KB stack limit",
                    ),
                );
            }
        };
        const _: () = if !(0usize + core::mem::size_of::<Account<Mint>>()
            + core::mem::size_of::<Account<EscrowState>>()
            + core::mem::size_of::<Account<TokenAccount>>()
            + core::mem::size_of::<Account<TokenAccount>>() <= 1700)
        {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: this Accounts struct\'s data-carrying fields (mint_a, escrow_state, vault_token_account, maker_token_account_a) sum to more than the safe stack budget for `load_and_validate` — box one or more of them as `Box<Account<T>>`",
                    ),
                );
            }
        };
        impl<'a> naclac_lang::prelude::LoadableAccounts<'a> for Cancel {
            type Payload = Self;
            fn load_and_validate<const __NACLAC_IX_DISC_LEN: usize>(
                program_id: &naclac_lang::prelude::Address,
                accounts: &'a [naclac_lang::prelude::AccountType],
                instruction_data: &[u8],
                __duplicates: Option<&naclac_lang::prelude::AccountBitvec>,
            ) -> naclac_lang::prelude::ValidationResult<'a, Self::Payload, CancelBumps> {
                if accounts.len() < 7usize {
                    return Err(
                        naclac_lang::prelude::NaclacError::NotEnoughAccountKeys.err(0),
                    );
                }
                if let Some(__dups) = __duplicates {
                    if __dups.intersects(&Self::MUT_MASK) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount
                                .err(0),
                        );
                    }
                }
                let __views = accounts;
                let mut __r: &[u8] = &instruction_data[__NACLAC_IX_DISC_LEN..];
                let seed = <u64 as naclac_lang::prelude::BorshDeserialize>::deserialize(
                        &mut __r,
                    )
                    .map_err(|_| {
                        naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)
                    })?;
                let _ = &seed;
                let info = &__views[0usize];
                {
                    if !info.is_writable {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintMut.err(0usize),
                        );
                    }
                }
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
                    let __loaded = <Signer as naclac_lang::prelude::NaclacAccount>::try_from_mut(
                        info,
                        0usize,
                    )?;
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
                {
                    if !info.is_writable {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintMut.err(2usize),
                        );
                    }
                }
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
                let __pda_program_owned = naclac_lang::prelude::ToAddress::address(
                    &program_id,
                );
                let __pda_program: &naclac_lang::prelude::Address = &__pda_program_owned;
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
                    if (escrow_state).address()
                        != naclac_lang::prelude::ToAddress::address(&expected_pda)
                    {
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
                {
                    if !info.is_writable {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintMut.err(3usize),
                        );
                    }
                }
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
                {
                    if !info.is_writable {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintMut.err(4usize),
                        );
                    }
                }
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
                let remaining_accounts = &accounts[7usize..];
                let mut __me = Self {
                    maker,
                    mint_a,
                    escrow_state,
                    vault_token_account,
                    maker_token_account_a,
                    token_program,
                    system_program,
                };
                {
                    let mut __rem_check_idx = 7usize;
                    for __rem_acc in remaining_accounts {
                        {
                            let __decl = &__me.maker;
                            let __rem_addr = naclac_lang::prelude::ToAddress::address(
                                __rem_acc,
                            );
                            if naclac_lang::prelude::ToAddress::address(__decl)
                                == __rem_addr
                            {
                                return Err(
                                    naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount
                                        .err(__rem_check_idx),
                                );
                            }
                        }
                        {
                            let __decl = &__me.escrow_state;
                            let __rem_addr = naclac_lang::prelude::ToAddress::address(
                                __rem_acc,
                            );
                            if naclac_lang::prelude::ToAddress::address(__decl)
                                == __rem_addr
                            {
                                return Err(
                                    naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount
                                        .err(__rem_check_idx),
                                );
                            }
                        }
                        {
                            let __decl = &__me.vault_token_account;
                            let __rem_addr = naclac_lang::prelude::ToAddress::address(
                                __rem_acc,
                            );
                            if naclac_lang::prelude::ToAddress::address(__decl)
                                == __rem_addr
                            {
                                return Err(
                                    naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount
                                        .err(__rem_check_idx),
                                );
                            }
                        }
                        {
                            let __decl = &__me.maker_token_account_a;
                            let __rem_addr = naclac_lang::prelude::ToAddress::address(
                                __rem_acc,
                            );
                            if naclac_lang::prelude::ToAddress::address(__decl)
                                == __rem_addr
                            {
                                return Err(
                                    naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount
                                        .err(__rem_check_idx),
                                );
                            }
                        }
                        __rem_check_idx += 1;
                    }
                }
                let __val = __me.escrow_state.mint_a;
                let __target_key = (__me.mint_a).address();
                if __val != __target_key {
                    return Err(EscrowError::InvalidMint.into());
                }
                Ok((
                    __me,
                    remaining_accounts,
                    CancelBumps {
                        escrow_state: __bump_escrow_state,
                    },
                ))
            }
            #[inline(always)]
            fn as_mut_payload(payload: &mut Self::Payload) -> &mut Self {
                payload
            }
            #[inline(always)]
            fn teardown_payload(
                payload: &mut Self::Payload,
                program_id: &naclac_lang::prelude::Address,
                bumps: &CancelBumps,
            ) -> Result<(), naclac_lang::prelude::ProgramError> {
                payload.teardown(program_id, bumps)
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
                    self.escrow_state.data.bump = bumps.escrow_state;
                }
                naclac_lang::prelude::NaclacAccount::exit(&self.maker, program_id)?;
                naclac_lang::prelude::NaclacAccount::exit(
                    &self.escrow_state,
                    program_id,
                )?;
                naclac_lang::prelude::NaclacAccount::exit(
                    &self.vault_token_account,
                    program_id,
                )?;
                naclac_lang::prelude::NaclacAccount::exit(
                    &self.maker_token_account_a,
                    program_id,
                )?;
                Ok(())
            }
        }
        const _: () = if !(core::mem::size_of::<Account<Mint>>() <= 300) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: field `mint_a` is large for a single stack frame — wrap it as `Box<Account<Mint>>` to keep `load_and_validate` under the SBF 4KB stack limit",
                    ),
                );
            }
        };
        const _: () = if !(core::mem::size_of::<Account<Mint>>() <= 300) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: field `mint_b` is large for a single stack frame — wrap it as `Box<Account<Mint>>` to keep `load_and_validate` under the SBF 4KB stack limit",
                    ),
                );
            }
        };
        const _: () = if !(core::mem::size_of::<Account<EscrowState>>() <= 300) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: field `escrow_state` is large for a single stack frame — wrap it as `Box<Account<EscrowState>>` to keep `load_and_validate` under the SBF 4KB stack limit",
                    ),
                );
            }
        };
        const _: () = if !(core::mem::size_of::<Account<TokenAccount>>() <= 300) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: field `vault_token_account` is large for a single stack frame — wrap it as `Box<Account<TokenAccount>>` to keep `load_and_validate` under the SBF 4KB stack limit",
                    ),
                );
            }
        };
        const _: () = if !(core::mem::size_of::<Account<TokenAccount>>() <= 300) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: field `maker_token_account_a` is large for a single stack frame — wrap it as `Box<Account<TokenAccount>>` to keep `load_and_validate` under the SBF 4KB stack limit",
                    ),
                );
            }
        };
        const _: () = if !(0usize + core::mem::size_of::<Account<Mint>>()
            + core::mem::size_of::<Account<Mint>>()
            + core::mem::size_of::<Account<EscrowState>>()
            + core::mem::size_of::<Account<TokenAccount>>()
            + core::mem::size_of::<Account<TokenAccount>>() <= 1700)
        {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: this Accounts struct\'s data-carrying fields (mint_a, mint_b, escrow_state, vault_token_account, maker_token_account_a) sum to more than the safe stack budget for `load_and_validate` — box one or more of them as `Box<Account<T>>`",
                    ),
                );
            }
        };
        impl<'a> naclac_lang::prelude::LoadableAccounts<'a> for Make {
            type Payload = Self;
            fn load_and_validate<const __NACLAC_IX_DISC_LEN: usize>(
                program_id: &naclac_lang::prelude::Address,
                accounts: &'a [naclac_lang::prelude::AccountType],
                instruction_data: &[u8],
                __duplicates: Option<&naclac_lang::prelude::AccountBitvec>,
            ) -> naclac_lang::prelude::ValidationResult<'a, Self::Payload, MakeBumps> {
                if accounts.len() < 8usize {
                    return Err(
                        naclac_lang::prelude::NaclacError::NotEnoughAccountKeys.err(0),
                    );
                }
                if let Some(__dups) = __duplicates {
                    if __dups.intersects(&Self::MUT_MASK) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount
                                .err(0),
                        );
                    }
                }
                let __views = accounts;
                let mut __r: &[u8] = &instruction_data[__NACLAC_IX_DISC_LEN..];
                let seed = <u64 as naclac_lang::prelude::BorshDeserialize>::deserialize(
                        &mut __r,
                    )
                    .map_err(|_| {
                        naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)
                    })?;
                let _ = &seed;
                let info = &__views[0usize];
                {
                    if !info.is_writable {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintMut.err(0usize),
                        );
                    }
                }
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
                    let __loaded = <Signer as naclac_lang::prelude::NaclacAccount>::try_from_mut(
                        info,
                        0usize,
                    )?;
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
                {
                    if !info.is_writable {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintMut.err(3usize),
                        );
                    }
                }
                let __seed_escrow_state_0 = &SEED_ESCROW;
                let __seed_escrow_state_1 = naclac_lang::prelude::ToAddress::address(
                    &(maker.address()),
                );
                let __seed_escrow_state_2 = &&seed.to_le_bytes();
                let __pda_program_owned = naclac_lang::prelude::ToAddress::address(
                    &program_id,
                );
                let __pda_program: &naclac_lang::prelude::Address = &__pda_program_owned;
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
                    let __key = (info).address();
                    if __key != naclac_lang::prelude::ToAddress::address(&expected_pda) {
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
                        let __data = info.try_borrow_data()?;
                        if __disc_len == 8 {
                            __data.len() >= 8 && __data[0..8] == [0u8; 8]
                        } else {
                            __disc_len > 0 && __data.len() >= __disc_len
                                && __data[..__disc_len].iter().all(|__b| *__b == 0)
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
                        let lamports = naclac_lang::prelude::const_rent_lamports(
                            ((EscrowState::SPACE)) as usize,
                        );
                        let __expected_bump_arr = [expected_bump];
                        let __seed_escrow_state_0 = &SEED_ESCROW;
                        let __seed_escrow_state_1 = naclac_lang::prelude::ToAddress::address(
                            &(maker.address()),
                        );
                        let __seed_escrow_state_2 = &&seed.to_le_bytes();
                        let signer_seeds: &[&[&[u8]]] = &[
                            &[
                                naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                                    &__seed_escrow_state_0,
                                ),
                                naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                                    &__seed_escrow_state_1,
                                ),
                                naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                                    &__seed_escrow_state_2,
                                ),
                                &__expected_bump_arr,
                            ],
                        ];
                        let __sys_info = &accounts[7usize];
                        if (__sys_info).address()
                            != naclac_lang::prelude::SYSTEM_PROGRAM_ID
                        {
                            return Err(
                                naclac_lang::prelude::NaclacError::ProgramIdMismatch
                                    .err(7usize),
                            );
                        }
                        let __payer_info_wrapper = naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &accounts[0usize],
                        );
                        if info.data_is_empty() {
                            naclac_lang::prelude::system_program::create_account_signed(
                                naclac_lang::prelude::CpiHandleMut {
                                    info: __payer_info_wrapper,
                                    _phantom: core::marker::PhantomData,
                                },
                                naclac_lang::prelude::CpiHandleMut {
                                    info: info.clone(),
                                    _phantom: core::marker::PhantomData,
                                },
                                naclac_lang::prelude::CpiHandle {
                                    info: __sys_info.clone(),
                                    _phantom: core::marker::PhantomData,
                                },
                                lamports,
                                ((EscrowState::SPACE)) as u64,
                                program_id,
                                signer_seeds,
                            )?;
                        }
                        let mut __account_data = info.try_borrow_mut_data()?;
                        __account_data
                            .get_mut(..__disc_len)
                            .ok_or_else(|| {
                                naclac_lang::prelude::NaclacError::AccountDataTooSmall
                                    .err(3usize)
                            })?
                            .copy_from_slice(
                                <EscrowState as naclac_lang::prelude::Discriminator>::DISCRIMINATOR,
                            );
                    }
                }
                let mut escrow_state = <Account<
                    EscrowState,
                > as naclac_lang::prelude::NaclacAccount>::try_from(info, 3usize)?;
                let info = &__views[4usize];
                {
                    if !info.is_writable {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintMut.err(4usize),
                        );
                    }
                }
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
                {
                    if !info.is_writable {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintMut.err(5usize),
                        );
                    }
                }
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
                let remaining_accounts = &accounts[8usize..];
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
                {
                    let mut __rem_check_idx = 8usize;
                    for __rem_acc in remaining_accounts {
                        {
                            let __decl = &__me.maker;
                            let __rem_addr = naclac_lang::prelude::ToAddress::address(
                                __rem_acc,
                            );
                            if naclac_lang::prelude::ToAddress::address(__decl)
                                == __rem_addr
                            {
                                return Err(
                                    naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount
                                        .err(__rem_check_idx),
                                );
                            }
                        }
                        {
                            let __decl = &__me.escrow_state;
                            let __rem_addr = naclac_lang::prelude::ToAddress::address(
                                __rem_acc,
                            );
                            if naclac_lang::prelude::ToAddress::address(__decl)
                                == __rem_addr
                            {
                                return Err(
                                    naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount
                                        .err(__rem_check_idx),
                                );
                            }
                        }
                        {
                            let __decl = &__me.vault_token_account;
                            let __rem_addr = naclac_lang::prelude::ToAddress::address(
                                __rem_acc,
                            );
                            if naclac_lang::prelude::ToAddress::address(__decl)
                                == __rem_addr
                            {
                                return Err(
                                    naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount
                                        .err(__rem_check_idx),
                                );
                            }
                        }
                        {
                            let __decl = &__me.maker_token_account_a;
                            let __rem_addr = naclac_lang::prelude::ToAddress::address(
                                __rem_acc,
                            );
                            if naclac_lang::prelude::ToAddress::address(__decl)
                                == __rem_addr
                            {
                                return Err(
                                    naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount
                                        .err(__rem_check_idx),
                                );
                            }
                        }
                        __rem_check_idx += 1;
                    }
                }
                Ok((
                    __me,
                    remaining_accounts,
                    MakeBumps {
                        escrow_state: __bump_escrow_state,
                    },
                ))
            }
            #[inline(always)]
            fn as_mut_payload(payload: &mut Self::Payload) -> &mut Self {
                payload
            }
            #[inline(always)]
            fn teardown_payload(
                payload: &mut Self::Payload,
                program_id: &naclac_lang::prelude::Address,
                bumps: &MakeBumps,
            ) -> Result<(), naclac_lang::prelude::ProgramError> {
                payload.teardown(program_id, bumps)
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
                naclac_lang::prelude::NaclacAccount::exit(&self.taker, program_id)?;
                naclac_lang::prelude::NaclacAccount::exit(&self.maker, program_id)?;
                naclac_lang::prelude::NaclacAccount::exit(
                    &self.escrow_state,
                    program_id,
                )?;
                naclac_lang::prelude::NaclacAccount::exit(
                    &self.vault_token_account,
                    program_id,
                )?;
                naclac_lang::prelude::NaclacAccount::exit(
                    &self.taker_token_account_a,
                    program_id,
                )?;
                naclac_lang::prelude::NaclacAccount::exit(
                    &self.taker_token_account_b,
                    program_id,
                )?;
                naclac_lang::prelude::NaclacAccount::exit(
                    &self.maker_token_account_b,
                    program_id,
                )?;
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
                    if target_info.owner != program_id {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintOwner
                                .err(4usize),
                        );
                    }
                    let dest_starting_lamports = dest_info.lamports();
                    **dest_info.lamports.borrow_mut() = dest_starting_lamports
                        .checked_add(target_info.lamports())
                        .unwrap();
                    **target_info.lamports.borrow_mut() = 0;
                    target_info.assign(&naclac_lang::prelude::SYSTEM_PROGRAM_ID);
                    target_info.data.borrow_mut().fill(0);
                }
                Ok(())
            }
        }
        const _: () = if !(core::mem::size_of::<Account<Mint>>() <= 300) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: field `mint_a` is large for a single stack frame — wrap it as `Box<Account<Mint>>` to keep `load_and_validate` under the SBF 4KB stack limit",
                    ),
                );
            }
        };
        const _: () = if !(core::mem::size_of::<Account<Mint>>() <= 300) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: field `mint_b` is large for a single stack frame — wrap it as `Box<Account<Mint>>` to keep `load_and_validate` under the SBF 4KB stack limit",
                    ),
                );
            }
        };
        const _: () = if !(core::mem::size_of::<Account<EscrowState>>() <= 300) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: field `escrow_state` is large for a single stack frame — wrap it as `Box<Account<EscrowState>>` to keep `load_and_validate` under the SBF 4KB stack limit",
                    ),
                );
            }
        };
        const _: () = if !(core::mem::size_of::<Account<TokenAccount>>() <= 300) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: field `vault_token_account` is large for a single stack frame — wrap it as `Box<Account<TokenAccount>>` to keep `load_and_validate` under the SBF 4KB stack limit",
                    ),
                );
            }
        };
        const _: () = if !(core::mem::size_of::<Account<TokenAccount>>() <= 300) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: field `taker_token_account_a` is large for a single stack frame — wrap it as `Box<Account<TokenAccount>>` to keep `load_and_validate` under the SBF 4KB stack limit",
                    ),
                );
            }
        };
        const _: () = if !(core::mem::size_of::<Account<TokenAccount>>() <= 300) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: field `taker_token_account_b` is large for a single stack frame — wrap it as `Box<Account<TokenAccount>>` to keep `load_and_validate` under the SBF 4KB stack limit",
                    ),
                );
            }
        };
        const _: () = if !(core::mem::size_of::<Account<TokenAccount>>() <= 300) {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: field `maker_token_account_b` is large for a single stack frame — wrap it as `Box<Account<TokenAccount>>` to keep `load_and_validate` under the SBF 4KB stack limit",
                    ),
                );
            }
        };
        const _: () = if !(0usize + core::mem::size_of::<Account<Mint>>()
            + core::mem::size_of::<Account<Mint>>()
            + core::mem::size_of::<Account<EscrowState>>()
            + core::mem::size_of::<Account<TokenAccount>>()
            + core::mem::size_of::<Account<TokenAccount>>()
            + core::mem::size_of::<Account<TokenAccount>>()
            + core::mem::size_of::<Account<TokenAccount>>() <= 1700)
        {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "naclac: this Accounts struct\'s data-carrying fields (mint_a, mint_b, escrow_state, vault_token_account, taker_token_account_a, taker_token_account_b, maker_token_account_b) sum to more than the safe stack budget for `load_and_validate` — box one or more of them as `Box<Account<T>>`",
                    ),
                );
            }
        };
        impl<'a> naclac_lang::prelude::LoadableAccounts<'a> for Take {
            type Payload = Self;
            fn load_and_validate<const __NACLAC_IX_DISC_LEN: usize>(
                program_id: &naclac_lang::prelude::Address,
                accounts: &'a [naclac_lang::prelude::AccountType],
                instruction_data: &[u8],
                __duplicates: Option<&naclac_lang::prelude::AccountBitvec>,
            ) -> naclac_lang::prelude::ValidationResult<'a, Self::Payload, TakeBumps> {
                if accounts.len() < 11usize {
                    return Err(
                        naclac_lang::prelude::NaclacError::NotEnoughAccountKeys.err(0),
                    );
                }
                if let Some(__dups) = __duplicates {
                    if __dups.intersects(&Self::MUT_MASK) {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount
                                .err(0),
                        );
                    }
                }
                let __views = accounts;
                let mut __r: &[u8] = &instruction_data[__NACLAC_IX_DISC_LEN..];
                let seed = <u64 as naclac_lang::prelude::BorshDeserialize>::deserialize(
                        &mut __r,
                    )
                    .map_err(|_| {
                        naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)
                    })?;
                let _ = &seed;
                let info = &__views[0usize];
                {
                    if !info.is_writable {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintMut.err(0usize),
                        );
                    }
                }
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
                    let __loaded = <Signer as naclac_lang::prelude::NaclacAccount>::try_from_mut(
                        info,
                        0usize,
                    )?;
                    __loaded
                };
                let info = &__views[1usize];
                {
                    if !info.is_writable {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintMut.err(1usize),
                        );
                    }
                }
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
                {
                    if !info.is_writable {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintMut.err(4usize),
                        );
                    }
                }
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
                let __pda_program_owned = naclac_lang::prelude::ToAddress::address(
                    &program_id,
                );
                let __pda_program: &naclac_lang::prelude::Address = &__pda_program_owned;
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
                    if (escrow_state).address()
                        != naclac_lang::prelude::ToAddress::address(&expected_pda)
                    {
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
                {
                    if !info.is_writable {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintMut.err(5usize),
                        );
                    }
                }
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
                {
                    if !info.is_writable {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintMut.err(6usize),
                        );
                    }
                }
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
                {
                    if !info.is_writable {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintMut.err(7usize),
                        );
                    }
                }
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
                {
                    if !info.is_writable {
                        return Err(
                            naclac_lang::prelude::NaclacError::ConstraintMut.err(8usize),
                        );
                    }
                }
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
                let remaining_accounts = &accounts[11usize..];
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
                {
                    let mut __rem_check_idx = 11usize;
                    for __rem_acc in remaining_accounts {
                        {
                            let __decl = &__me.taker;
                            let __rem_addr = naclac_lang::prelude::ToAddress::address(
                                __rem_acc,
                            );
                            if naclac_lang::prelude::ToAddress::address(__decl)
                                == __rem_addr
                            {
                                return Err(
                                    naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount
                                        .err(__rem_check_idx),
                                );
                            }
                        }
                        {
                            let __decl = &__me.maker;
                            let __rem_addr = naclac_lang::prelude::ToAddress::address(
                                __rem_acc,
                            );
                            if naclac_lang::prelude::ToAddress::address(__decl)
                                == __rem_addr
                            {
                                return Err(
                                    naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount
                                        .err(__rem_check_idx),
                                );
                            }
                        }
                        {
                            let __decl = &__me.escrow_state;
                            let __rem_addr = naclac_lang::prelude::ToAddress::address(
                                __rem_acc,
                            );
                            if naclac_lang::prelude::ToAddress::address(__decl)
                                == __rem_addr
                            {
                                return Err(
                                    naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount
                                        .err(__rem_check_idx),
                                );
                            }
                        }
                        {
                            let __decl = &__me.vault_token_account;
                            let __rem_addr = naclac_lang::prelude::ToAddress::address(
                                __rem_acc,
                            );
                            if naclac_lang::prelude::ToAddress::address(__decl)
                                == __rem_addr
                            {
                                return Err(
                                    naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount
                                        .err(__rem_check_idx),
                                );
                            }
                        }
                        {
                            let __decl = &__me.taker_token_account_a;
                            let __rem_addr = naclac_lang::prelude::ToAddress::address(
                                __rem_acc,
                            );
                            if naclac_lang::prelude::ToAddress::address(__decl)
                                == __rem_addr
                            {
                                return Err(
                                    naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount
                                        .err(__rem_check_idx),
                                );
                            }
                        }
                        {
                            let __decl = &__me.taker_token_account_b;
                            let __rem_addr = naclac_lang::prelude::ToAddress::address(
                                __rem_acc,
                            );
                            if naclac_lang::prelude::ToAddress::address(__decl)
                                == __rem_addr
                            {
                                return Err(
                                    naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount
                                        .err(__rem_check_idx),
                                );
                            }
                        }
                        {
                            let __decl = &__me.maker_token_account_b;
                            let __rem_addr = naclac_lang::prelude::ToAddress::address(
                                __rem_acc,
                            );
                            if naclac_lang::prelude::ToAddress::address(__decl)
                                == __rem_addr
                            {
                                return Err(
                                    naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount
                                        .err(__rem_check_idx),
                                );
                            }
                        }
                        __rem_check_idx += 1;
                    }
                }
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
                    remaining_accounts,
                    TakeBumps {
                        escrow_state: __bump_escrow_state,
                    },
                ))
            }
            #[inline(always)]
            fn as_mut_payload(payload: &mut Self::Payload) -> &mut Self {
                payload
            }
            #[inline(always)]
            fn teardown_payload(
                payload: &mut Self::Payload,
                program_id: &naclac_lang::prelude::Address,
                bumps: &TakeBumps,
            ) -> Result<(), naclac_lang::prelude::ProgramError> {
                payload.teardown(program_id, bumps)
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
        #[borsh(crate = "naclac_lang::prelude::borsh")]
        pub struct EscrowState {
            pub maker: Address,
            pub mint_a: Address,
            pub mint_b: Address,
            pub amount_a: u64,
            pub amount_b: u64,
            pub bump: u8,
        }
        #[automatically_derived]
        impl ::core::clone::Clone for EscrowState {
            #[inline]
            fn clone(&self) -> EscrowState {
                EscrowState {
                    maker: ::core::clone::Clone::clone(&self.maker),
                    mint_a: ::core::clone::Clone::clone(&self.mint_a),
                    mint_b: ::core::clone::Clone::clone(&self.mint_b),
                    amount_a: ::core::clone::Clone::clone(&self.amount_a),
                    amount_b: ::core::clone::Clone::clone(&self.amount_b),
                    bump: ::core::clone::Clone::clone(&self.bump),
                }
            }
        }
        #[automatically_derived]
        impl naclac_lang::prelude::borsh::ser::BorshSerialize for EscrowState {
            fn serialize<__W: naclac_lang::prelude::borsh::io::Write>(
                &self,
                writer: &mut __W,
            ) -> ::core::result::Result<(), naclac_lang::prelude::borsh::io::Error> {
                naclac_lang::prelude::borsh::BorshSerialize::serialize(
                    &self.maker,
                    writer,
                )?;
                naclac_lang::prelude::borsh::BorshSerialize::serialize(
                    &self.mint_a,
                    writer,
                )?;
                naclac_lang::prelude::borsh::BorshSerialize::serialize(
                    &self.mint_b,
                    writer,
                )?;
                naclac_lang::prelude::borsh::BorshSerialize::serialize(
                    &self.amount_a,
                    writer,
                )?;
                naclac_lang::prelude::borsh::BorshSerialize::serialize(
                    &self.amount_b,
                    writer,
                )?;
                naclac_lang::prelude::borsh::BorshSerialize::serialize(
                    &self.bump,
                    writer,
                )?;
                Ok(())
            }
        }
        #[automatically_derived]
        impl naclac_lang::prelude::borsh::de::BorshDeserialize for EscrowState {
            fn deserialize_reader<__R: naclac_lang::prelude::borsh::io::Read>(
                reader: &mut __R,
            ) -> ::core::result::Result<Self, naclac_lang::prelude::borsh::io::Error> {
                Ok(Self {
                    maker: naclac_lang::prelude::borsh::BorshDeserialize::deserialize_reader(
                        reader,
                    )?,
                    mint_a: naclac_lang::prelude::borsh::BorshDeserialize::deserialize_reader(
                        reader,
                    )?,
                    mint_b: naclac_lang::prelude::borsh::BorshDeserialize::deserialize_reader(
                        reader,
                    )?,
                    amount_a: naclac_lang::prelude::borsh::BorshDeserialize::deserialize_reader(
                        reader,
                    )?,
                    amount_b: naclac_lang::prelude::borsh::BorshDeserialize::deserialize_reader(
                        reader,
                    )?,
                    bump: naclac_lang::prelude::borsh::BorshDeserialize::deserialize_reader(
                        reader,
                    )?,
                })
            }
        }
        impl EscrowState {
            pub const DISCRIMINATOR: &'static [u8] = &[
                19u8, 90u8, 148u8, 111u8, 55u8, 130u8, 229u8, 108u8,
            ];
            pub const SPACE: usize = Self::DISCRIMINATOR.len()
                + <Address as naclac_lang::prelude::SerializedBounds>::MAX_SIZE
                + <Address as naclac_lang::prelude::SerializedBounds>::MAX_SIZE
                + <Address as naclac_lang::prelude::SerializedBounds>::MAX_SIZE
                + <u64 as naclac_lang::prelude::SerializedBounds>::MAX_SIZE
                + <u64 as naclac_lang::prelude::SerializedBounds>::MAX_SIZE
                + <u8 as naclac_lang::prelude::SerializedBounds>::MAX_SIZE;
            pub fn try_deserialize(
                data: &mut &[u8],
            ) -> naclac_lang::prelude::Result<Self> {
                let disc_len = Self::DISCRIMINATOR.len();
                if data.len() < disc_len {
                    return Err(
                        naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(0),
                    );
                }
                if &data[..disc_len] != Self::DISCRIMINATOR {
                    return Err(
                        naclac_lang::prelude::NaclacError::InvalidAccountDiscriminator
                            .err(0),
                    );
                }
                let mut reader = &data[disc_len..];
                naclac_lang::prelude::BorshDeserialize::deserialize(&mut reader)
                    .map_err(|_| {
                        naclac_lang::prelude::NaclacError::DeserializationFailed.err(0)
                    })
            }
            pub fn try_serialize<W: naclac_lang::prelude::borsh::io::Write>(
                &self,
                writer: &mut W,
            ) -> naclac_lang::prelude::Result<()> {
                writer
                    .write_all(Self::DISCRIMINATOR)
                    .map_err(|_| {
                        naclac_lang::prelude::NaclacError::SerializationFailed.err(0)
                    })?;
                naclac_lang::prelude::BorshSerialize::serialize(self, writer)
                    .map_err(|_| {
                        naclac_lang::prelude::NaclacError::SerializationFailed.err(0)
                    })
            }
        }
        impl naclac_lang::prelude::Discriminator for EscrowState {
            const DISCRIMINATOR: &'static [u8] = &[
                19u8, 90u8, 148u8, 111u8, 55u8, 130u8, 229u8, 108u8,
            ];
            fn within_max_len(&self) -> bool {
                true
                    && naclac_lang::prelude::SerializedBounds::within_bounds(&self.maker)
                    && naclac_lang::prelude::SerializedBounds::within_bounds(
                        &self.mint_a,
                    )
                    && naclac_lang::prelude::SerializedBounds::within_bounds(
                        &self.mint_b,
                    )
                    && naclac_lang::prelude::SerializedBounds::within_bounds(
                        &self.amount_a,
                    )
                    && naclac_lang::prelude::SerializedBounds::within_bounds(
                        &self.amount_b,
                    )
                    && naclac_lang::prelude::SerializedBounds::within_bounds(&self.bump)
            }
        }
    }
    pub use escrow_state::*;
}
use instructions::*;
const _: bool = naclac_lang::__NACLAC_LANG_HAS_SOLANA_RUNTIME;
const _: bool = naclac_lang::__NACLAC_LANG_HAS_BORSH_LAYOUT;
#[doc(hidden)]
pub const __NACLAC_EVENT_DISPATCH_TAG: &[u8] = &[
    34u8, 131u8, 189u8, 8u8, 189u8, 141u8, 224u8, 48u8,
];
#[doc(hidden)]
pub const __NACLAC_EVENT_CPI_IN_USE: bool = escrow_borsh::__NACLAC_NEEDS_EVENT_CPI;
#[no_mangle]
pub unsafe extern "C" fn entrypoint(input: *mut u8) -> u64 {
    let Some((program_id, accounts, instruction_data)) = (unsafe {
        ::naclac_core::entrypoint::deserialize(input)
    }) else {
        return ::naclac_core::prelude::ProgramError::InvalidAccountData.into();
    };
    let result = match process_instruction(program_id, &accounts, instruction_data) {
        Ok(()) => ::naclac_core::entrypoint::SUCCESS,
        Err(error) => error.into(),
    };
    ::core::mem::forget(accounts);
    result
}
/// Handles an `emit_cpi!` self-invocation: succeeds only when the
/// first account is the program's event authority PDA and signed.
#[inline(always)]
fn __naclac_receive_event_cpi(
    accounts: &[naclac_lang::prelude::AccountType],
) -> naclac_lang::prelude::Result {
    const _: &[u8] = b"use naclac_lang::prelude::*;\n\ndeclare_id!(\"G5gui4RgZH8tjr6M6Js2yJqTcG9tVukUz8p27BNonEgT\");\n\npub mod constants;\npub mod errors;\npub mod events;\npub mod instructions;\npub mod state;\n\nuse instructions::*;\n\n#[program]\npub mod escrow_borsh {\n    pub fn make(ctx: Context<Make>, seed: u64, amount_a: u64, amount_b: u64) -> Result {\n        make::make(ctx, seed, amount_a, amount_b)\n    }\n\n    pub fn take(ctx: Context<Take>, seed: u64) -> Result {\n        take::take(ctx, seed)\n    }\n\n    pub fn cancel(ctx: Context<Cancel>, seed: u64) -> Result {\n        cancel::cancel(ctx, seed)\n    }\n}\n";
    {
        let __ea_slice: &[naclac_lang::prelude::AccountType] = accounts;
        let Some(__ea) = __ea_slice.first() else {
            return Err(naclac_lang::prelude::NaclacError::NotEnoughAccountKeys.err(0));
        };
        if !__ea.is_signer {
            return Err(naclac_lang::prelude::NaclacError::ConstraintSigner.err(0));
        }
        if __ea.address()
            != naclac_lang::prelude::Address::new_from_array([
                147u8, 183u8, 182u8, 209u8, 55u8, 31u8, 110u8, 10u8, 243u8, 157u8, 204u8,
                147u8, 12u8, 5u8, 55u8, 133u8, 162u8, 2u8, 65u8, 137u8, 246u8, 115u8,
                254u8, 87u8, 12u8, 1u8, 60u8, 42u8, 227u8, 236u8, 37u8, 205u8,
            ])
        {
            return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(0));
        }
        Ok(())
    }
}
pub fn process_instruction(
    program_id: &naclac_lang::solana_address::Address,
    accounts: &[naclac_lang::solana_account_info::AccountInfo<'_>],
    instruction_data: &[u8],
) -> naclac_lang::prelude::Result {
    unsafe {
        let program_id: &'static naclac_lang::prelude::Address = core::mem::transmute(
            program_id,
        );
        let accounts: &'static [naclac_lang::prelude::AccountInfoData] = core::mem::transmute(
            accounts,
        );
        let instruction_data: &'static [u8] = core::mem::transmute(instruction_data);
        process_instruction_inner(
            program_id,
            naclac_lang::prelude::account_handles(accounts),
            instruction_data,
        )
    }
}
#[inline(always)]
fn process_instruction_inner(
    program_id: &'static naclac_lang::prelude::Address,
    accounts: &'static [naclac_lang::prelude::AccountInfo],
    instruction_data: &'static [u8],
) -> naclac_lang::prelude::Result {
    if instruction_data.len() < 8usize {
        return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
    }
    let instruction_discriminator = u64::from_le_bytes(unsafe {
        *(instruction_data.as_ptr() as *const [u8; 8])
    });
    let mut __has_dups = false;
    let mut __dups = naclac_lang::prelude::AccountBitvec::default();
    for i in 0..accounts.len() {
        for j in 0..i {
            if core::ptr::eq(accounts[i].key, accounts[j].key)
                && accounts[i].key != program_id
            {
                __dups.set(i as u8);
                __dups.set(j as u8);
                __has_dups = true;
            }
        }
    }
    let __dups_opt = if __has_dups { Some(&__dups) } else { None };
    match instruction_discriminator {
        14222551101251052426u64 => {
            escrow_borsh::__dispatch_make(
                program_id,
                accounts,
                instruction_data,
                __dups_opt,
            )
        }
        2875141570291884693u64 => {
            escrow_borsh::__dispatch_take(
                program_id,
                accounts,
                instruction_data,
                __dups_opt,
            )
        }
        13753127788127181800u64 => {
            escrow_borsh::__dispatch_cancel(
                program_id,
                accounts,
                instruction_data,
                __dups_opt,
            )
        }
        3521970751638700834u64 if escrow_borsh::__NACLAC_NEEDS_EVENT_CPI => {
            __naclac_receive_event_cpi(accounts)
        }
        _ => Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)),
    }
}
pub mod escrow_borsh {
    use super::*;
    pub(crate) fn __dispatch_make(
        program_id: &naclac_lang::prelude::Address,
        accounts: &[naclac_lang::prelude::AccountInfo],
        instruction_data: &[u8],
        __duplicates: Option<&naclac_lang::prelude::AccountBitvec>,
    ) -> naclac_lang::prelude::Result {
        let mut __ix_offset: usize = 8usize;
        let seed = {
            let mut __r: &[u8] = &instruction_data[__ix_offset..];
            let __val = <u64 as naclac_lang::prelude::BorshDeserialize>::deserialize(
                    &mut __r,
                )
                .map_err(|_| {
                    naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)
                })?;
            __ix_offset = instruction_data.len() - __r.len();
            __val
        };
        let amount_a = {
            let mut __r: &[u8] = &instruction_data[__ix_offset..];
            let __val = <u64 as naclac_lang::prelude::BorshDeserialize>::deserialize(
                    &mut __r,
                )
                .map_err(|_| {
                    naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)
                })?;
            __ix_offset = instruction_data.len() - __r.len();
            __val
        };
        let amount_b = {
            let mut __r: &[u8] = &instruction_data[__ix_offset..];
            let __val = <u64 as naclac_lang::prelude::BorshDeserialize>::deserialize(
                    &mut __r,
                )
                .map_err(|_| {
                    naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)
                })?;
            __ix_offset = instruction_data.len() - __r.len();
            __val
        };
        let (mut accounts, remaining_accounts, bumps) = Make::load_and_validate::<
            8usize,
        >(program_id, accounts, instruction_data, __duplicates)?;
        let ctx = unsafe {
            naclac_lang::prelude::Context::new(
                program_id,
                Make::as_mut_payload(&mut accounts),
                remaining_accounts,
                bumps,
            )
        };
        let __result = escrow_borsh::make(ctx, seed, amount_a, amount_b);
        if __result.is_ok() {
            Make::teardown_payload(&mut accounts, program_id, &bumps)?;
        }
        __result.map(|_| ())
    }
    pub fn make(ctx: Context<Make>, seed: u64, amount_a: u64, amount_b: u64) -> Result {
        make::make(ctx, seed, amount_a, amount_b)
    }
    pub(crate) fn __dispatch_take(
        program_id: &naclac_lang::prelude::Address,
        accounts: &[naclac_lang::prelude::AccountInfo],
        instruction_data: &[u8],
        __duplicates: Option<&naclac_lang::prelude::AccountBitvec>,
    ) -> naclac_lang::prelude::Result {
        let mut __ix_offset: usize = 8usize;
        let seed = {
            let mut __r: &[u8] = &instruction_data[__ix_offset..];
            let __val = <u64 as naclac_lang::prelude::BorshDeserialize>::deserialize(
                    &mut __r,
                )
                .map_err(|_| {
                    naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)
                })?;
            __ix_offset = instruction_data.len() - __r.len();
            __val
        };
        let (mut accounts, remaining_accounts, bumps) = Take::load_and_validate::<
            8usize,
        >(program_id, accounts, instruction_data, __duplicates)?;
        let ctx = unsafe {
            naclac_lang::prelude::Context::new(
                program_id,
                Take::as_mut_payload(&mut accounts),
                remaining_accounts,
                bumps,
            )
        };
        let __result = escrow_borsh::take(ctx, seed);
        if __result.is_ok() {
            Take::teardown_payload(&mut accounts, program_id, &bumps)?;
        }
        __result.map(|_| ())
    }
    pub fn take(ctx: Context<Take>, seed: u64) -> Result {
        take::take(ctx, seed)
    }
    pub(crate) fn __dispatch_cancel(
        program_id: &naclac_lang::prelude::Address,
        accounts: &[naclac_lang::prelude::AccountInfo],
        instruction_data: &[u8],
        __duplicates: Option<&naclac_lang::prelude::AccountBitvec>,
    ) -> naclac_lang::prelude::Result {
        let mut __ix_offset: usize = 8usize;
        let seed = {
            let mut __r: &[u8] = &instruction_data[__ix_offset..];
            let __val = <u64 as naclac_lang::prelude::BorshDeserialize>::deserialize(
                    &mut __r,
                )
                .map_err(|_| {
                    naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)
                })?;
            __ix_offset = instruction_data.len() - __r.len();
            __val
        };
        let (mut accounts, remaining_accounts, bumps) = Cancel::load_and_validate::<
            8usize,
        >(program_id, accounts, instruction_data, __duplicates)?;
        let ctx = unsafe {
            naclac_lang::prelude::Context::new(
                program_id,
                Cancel::as_mut_payload(&mut accounts),
                remaining_accounts,
                bumps,
            )
        };
        let __result = escrow_borsh::cancel(ctx, seed);
        if __result.is_ok() {
            Cancel::teardown_payload(&mut accounts, program_id, &bumps)?;
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
