#![feature(prelude_import)]
#![no_std]
extern crate core;
#[prelude_import]
use core::prelude::rust_2021::*;
use naclac_lang::prelude::*;
pub const ID: ::naclac_lang::prelude::Address = unsafe {
    core::mem::transmute(
        ::solana_address::Address::from_str_const(
            "H1Hcpaf3iW9RHrc8bAYkrEcqfb3kTWRvgZceNBi8tWYX",
        ),
    )
};
pub fn id() -> ::naclac_lang::prelude::Address {
    ID
}
pub mod components {
    pub mod launch_record {
        use naclac_lang::prelude::*;
        #[repr(C)]
        pub struct LaunchRecord {
            pub creator: Address,
            pub mint: Address,
            pub amount_token: u64,
            pub amount_quote: u64,
        }
        impl core::clone::Clone for LaunchRecord {
            #[inline(always)]
            fn clone(&self) -> Self {
                *self
            }
        }
        impl core::marker::Copy for LaunchRecord {}
        unsafe impl naclac_lang::prelude::bytemuck::Pod for LaunchRecord {}
        unsafe impl naclac_lang::prelude::bytemuck::Zeroable for LaunchRecord {}
        impl LaunchRecord {
            pub const DISCRIMINATOR: [u8; 8] = [
                146u8, 240u8, 197u8, 204u8, 6u8, 6u8, 87u8, 75u8,
            ];
            pub const SPACE: usize = core::mem::size_of::<Self>() + 8;
            #[inline(always)]
            pub fn load(data: &[u8]) -> naclac_lang::prelude::Result<&Self> {
                if data.len() < Self::SPACE {
                    return Err(
                        naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(0),
                    );
                }
                if data[0..8] != Self::DISCRIMINATOR {
                    return Err(
                        naclac_lang::prelude::NaclacError::AccountNotInitialized.err(0),
                    );
                }
                Ok(naclac_lang::prelude::bytemuck::from_bytes(&data[8..Self::SPACE]))
            }
            #[inline(always)]
            pub fn load_mut(data: &mut [u8]) -> naclac_lang::prelude::Result<&mut Self> {
                if data.len() < Self::SPACE {
                    return Err(
                        naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(0),
                    );
                }
                if data[0..8] != Self::DISCRIMINATOR {
                    return Err(
                        naclac_lang::prelude::NaclacError::AccountNotInitialized.err(0),
                    );
                }
                Ok(
                    naclac_lang::prelude::bytemuck::from_bytes_mut(
                        &mut data[8..Self::SPACE],
                    ),
                )
            }
        }
        impl naclac_lang::prelude::NaclacZeroCopy for LaunchRecord {}
        impl naclac_lang::prelude::Discriminator for LaunchRecord {
            const DISCRIMINATOR: [u8; 8] = [
                146u8, 240u8, 197u8, 204u8, 6u8, 6u8, 87u8, 75u8,
            ];
        }
    }
    pub use launch_record::*;
}
pub mod instructions {
    pub mod launch_token {
        use naclac_lang::prelude::*;
        use crate::components::launch_record::LaunchRecord;
        use crate::constants::SEED_MINT;
        #[repr(C)]
        pub struct LaunchTokenArgs {
            pub id: u64,
            pub mint_bump: u8,
            pub launch_record_bump: u8,
            pub pool_bump: u8,
            pub decimals: u8,
            pub amount_token_pool: u64,
            pub amount_token_launcher: u64,
            pub amount_quote: u64,
        }
        #[automatically_derived]
        #[doc(hidden)]
        unsafe impl ::core::clone::TrivialClone for LaunchTokenArgs {}
        #[automatically_derived]
        impl ::core::clone::Clone for LaunchTokenArgs {
            #[inline]
            fn clone(&self) -> LaunchTokenArgs {
                let _: ::core::clone::AssertParamIsClone<u64>;
                let _: ::core::clone::AssertParamIsClone<u8>;
                *self
            }
        }
        #[automatically_derived]
        impl ::core::marker::Copy for LaunchTokenArgs {}
        unsafe impl naclac_lang::prelude::Pod for LaunchTokenArgs {}
        unsafe impl naclac_lang::prelude::Zeroable for LaunchTokenArgs {}
        impl naclac_lang::prelude::NaclacPod for LaunchTokenArgs {
            #[inline(always)]
            fn naclac_from_bytes(data: &[u8]) -> Self {
                unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Self) }
            }
            #[inline(always)]
            fn naclac_size() -> usize {
                core::mem::size_of::<Self>()
            }
        }
        #[instruction(args:LaunchTokenArgs)]
        pub struct LaunchToken {
            #[account(mut)]
            pub payer: Signer,
            pub quote_mint: Account<Mint>,
            #[account(
                mut,
                seeds = [SEED_MINT,
                &args.id.to_le_bytes()],
                bump = args.mint_bump
            )]
            pub mint: AccountInfo,
            #[account(
                init,
                payer = payer,
                seeds = [b"launch",
                payer.address().as_ref(),
                &args.id.to_le_bytes()],
                bump = args.launch_record_bump
            )]
            pub launch_record: Account<LaunchRecord>,
            #[account(mut)]
            pub launcher_token_a: Account<TokenAccount>,
            #[account(mut)]
            pub launcher_token_b: Account<TokenAccount>,
            #[account(mut)]
            pub launcher_lp: Account<TokenAccount>,
            #[account(mut)]
            pub payer_token_b: Account<TokenAccount>,
            #[account(mut)]
            pub pool_state: AccountInfo,
            #[account(mut)]
            pub pool_vault_a: AccountInfo,
            #[account(mut)]
            pub pool_vault_b: AccountInfo,
            #[account(mut)]
            pub pool_lp_mint: AccountInfo,
            pub amm_program: AccountInfo,
            pub token_program: Program<Token>,
            pub system_program: Program<System>,
        }
        pub struct LaunchTokenBumps {
            pub mint: u8,
            pub launch_record: u8,
        }
        #[automatically_derived]
        #[doc(hidden)]
        unsafe impl ::core::clone::TrivialClone for LaunchTokenBumps {}
        #[automatically_derived]
        impl ::core::clone::Clone for LaunchTokenBumps {
            #[inline]
            fn clone(&self) -> LaunchTokenBumps {
                let _: ::core::clone::AssertParamIsClone<u8>;
                *self
            }
        }
        #[automatically_derived]
        impl ::core::marker::Copy for LaunchTokenBumps {}
        #[automatically_derived]
        impl ::core::default::Default for LaunchTokenBumps {
            #[inline]
            fn default() -> LaunchTokenBumps {
                LaunchTokenBumps {
                    mint: ::core::default::Default::default(),
                    launch_record: ::core::default::Default::default(),
                }
            }
        }
        impl LaunchToken {
            pub fn to_account_metas(
                &self,
            ) -> [naclac_lang::prelude::AccountMeta; 15usize] {
                [
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.payer).address(),
                        is_signer: true,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.quote_mint).address(),
                        is_signer: false,
                        is_writable: false,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.mint).address(),
                        is_signer: false,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.launch_record).address(),
                        is_signer: false,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.launcher_token_a).address(),
                        is_signer: false,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.launcher_token_b).address(),
                        is_signer: false,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.launcher_lp).address(),
                        is_signer: false,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.payer_token_b).address(),
                        is_signer: false,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.pool_state).address(),
                        is_signer: false,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.pool_vault_a).address(),
                        is_signer: false,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.pool_vault_b).address(),
                        is_signer: false,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.pool_lp_mint).address(),
                        is_signer: false,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.amm_program).address(),
                        is_signer: false,
                        is_writable: false,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.token_program).address(),
                        is_signer: false,
                        is_writable: false,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.system_program).address(),
                        is_signer: false,
                        is_writable: false,
                    },
                ]
            }
        }
        impl LaunchToken {
            pub fn to_account_infos(
                &self,
            ) -> [naclac_lang::prelude::AccountInfo; 15usize] {
                [
                    naclac_lang::prelude::ToAccountInfo::to_account_info(&self.payer),
                    naclac_lang::prelude::ToAccountInfo::to_account_info(
                        &self.quote_mint,
                    ),
                    naclac_lang::prelude::ToAccountInfo::to_account_info(&self.mint),
                    naclac_lang::prelude::ToAccountInfo::to_account_info(
                        &self.launch_record,
                    ),
                    naclac_lang::prelude::ToAccountInfo::to_account_info(
                        &self.launcher_token_a,
                    ),
                    naclac_lang::prelude::ToAccountInfo::to_account_info(
                        &self.launcher_token_b,
                    ),
                    naclac_lang::prelude::ToAccountInfo::to_account_info(
                        &self.launcher_lp,
                    ),
                    naclac_lang::prelude::ToAccountInfo::to_account_info(
                        &self.payer_token_b,
                    ),
                    naclac_lang::prelude::ToAccountInfo::to_account_info(
                        &self.pool_state,
                    ),
                    naclac_lang::prelude::ToAccountInfo::to_account_info(
                        &self.pool_vault_a,
                    ),
                    naclac_lang::prelude::ToAccountInfo::to_account_info(
                        &self.pool_vault_b,
                    ),
                    naclac_lang::prelude::ToAccountInfo::to_account_info(
                        &self.pool_lp_mint,
                    ),
                    naclac_lang::prelude::ToAccountInfo::to_account_info(
                        &self.amm_program,
                    ),
                    naclac_lang::prelude::ToAccountInfo::to_account_info(
                        &self.token_program,
                    ),
                    naclac_lang::prelude::ToAccountInfo::to_account_info(
                        &self.system_program,
                    ),
                ]
            }
        }
        impl naclac_lang::prelude::Bumps for LaunchToken {
            type BumpsStruct = LaunchTokenBumps;
        }
        impl LaunchToken {
            /// Loads and validates the accounts provided to the instruction.
            ///
            /// ## Pinocchio path (pre-walked slice)
            /// Takes a pre-walked `&[AccountType]` slice provided by the entrypoint.
            /// Accounts were walked once globally before dispatch — O(1) slice indexing,
            /// no cursor creation, no lookup array allocation per instruction.
            ///
            /// ## Standard path (Borsh/solana-program)
            /// Takes the pre-built `&[AccountType]` slice (unchanged).
            pub fn load_and_validate(
                program_id: &naclac_lang::prelude::Address,
                __views: &[naclac_lang::prelude::AccountType],
                instruction_data: &[u8],
            ) -> Result<
                (Self, &'static [naclac_lang::prelude::AccountType], LaunchTokenBumps),
                naclac_lang::prelude::ProgramError,
            > {
                if __views.len() < 15usize {
                    return Err(naclac_lang::prelude::ProgramError::NotEnoughAccountKeys);
                }
                let accounts = __views;
                let mut __ix_offset: usize = 8;
                let args = {
                    let __sz = <LaunchTokenArgs as naclac_lang::prelude::NaclacPod>::naclac_size();
                    if instruction_data.len() < __ix_offset + __sz {
                        return Err(
                            naclac_lang::prelude::NaclacError::InvalidInstructionData
                                .err(0),
                        );
                    }
                    let __val = <LaunchTokenArgs as naclac_lang::prelude::NaclacPod>::naclac_from_bytes(
                        &instruction_data[__ix_offset..__ix_offset + __sz],
                    );
                    __ix_offset += __sz;
                    __val
                };
                let info = &__views[0usize];
                let mut payer: Signer = naclac_lang::prelude::Signer::try_from(
                    info,
                    0usize,
                )?;
                let info = &__views[1usize];
                let mut quote_mint: Account<Mint> = naclac_lang::prelude::Account::try_from(
                    info,
                    1usize,
                )?;
                let info = &__views[2usize];
                let mut mint: AccountInfo = info.clone();
                let __seed_mint_0 = &SEED_MINT;
                let __seed_mint_1 = &&args.id.to_le_bytes();
                let expected_bump = (args.mint_bump);
                let __expected_bump_arr = [expected_bump];
                let __pda_program = naclac_lang::prelude::ToAddress::address(
                    &program_id,
                );
                let inputs = [
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_mint_0,
                    ),
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_mint_1,
                    ),
                    &__expected_bump_arr,
                    __pda_program.as_ref(),
                    b"ProgramDerivedAddress",
                ];
                let expected_pda = {
                    extern "C" {
                        pub fn sol_sha256(
                            vals: *const u8,
                            val_len: u64,
                            hash_result: *mut u8,
                        ) -> u32;
                    }
                    let mut hash_result = [0u8; 32];
                    unsafe {
                        sol_sha256(
                            inputs.as_ptr() as *const u8,
                            inputs.len() as u64,
                            hash_result.as_mut_ptr(),
                        );
                    }
                    naclac_lang::prelude::Address::new_from_array(hash_result)
                };
                if (mint).address()
                    != naclac_lang::prelude::ToAddress::address(&expected_pda)
                {
                    return Err(
                        naclac_lang::prelude::NaclacError::ConstraintSeeds.err(2usize),
                    );
                }
                if expected_bump != (args.mint_bump) {
                    return Err(
                        naclac_lang::prelude::NaclacError::ConstraintSeeds.err(2usize),
                    );
                }
                let __bump_mint = expected_bump;
                let info = &__views[3usize];
                let __seed_launch_record_0 = &b"launch";
                let __seed_launch_record_1 = naclac_lang::prelude::ToAddress::address(
                    &(payer.address()),
                );
                let __seed_launch_record_2 = &&args.id.to_le_bytes();
                let expected_bump = (args.launch_record_bump);
                let __expected_bump_arr = [expected_bump];
                let __pda_program = naclac_lang::prelude::ToAddress::address(
                    &program_id,
                );
                let inputs = [
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_launch_record_0,
                    ),
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_launch_record_1,
                    ),
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_launch_record_2,
                    ),
                    &__expected_bump_arr,
                    __pda_program.as_ref(),
                    b"ProgramDerivedAddress",
                ];
                let expected_pda = {
                    extern "C" {
                        pub fn sol_sha256(
                            vals: *const u8,
                            val_len: u64,
                            hash_result: *mut u8,
                        ) -> u32;
                    }
                    let mut hash_result = [0u8; 32];
                    unsafe {
                        sol_sha256(
                            inputs.as_ptr() as *const u8,
                            inputs.len() as u64,
                            hash_result.as_mut_ptr(),
                        );
                    }
                    naclac_lang::prelude::Address::new_from_array(hash_result)
                };
                let __key = (info).address();
                if __key != naclac_lang::prelude::ToAddress::address(&expected_pda) {
                    return Err(
                        naclac_lang::prelude::NaclacError::ConstraintSeeds.err(3usize),
                    );
                }
                if expected_bump != (args.launch_record_bump) {
                    return Err(
                        naclac_lang::prelude::NaclacError::ConstraintSeeds.err(3usize),
                    );
                }
                let __bump_launch_record = expected_bump;
                let __needs_init = if info.data_is_empty() {
                    true
                } else {
                    {
                        let __data_ptr = info.view.data_ptr();
                        let __data_len = info.view.data_len();
                        __data_len >= 8 && unsafe { *(__data_ptr as *const u64) } == 0
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
                        let __seed_launch_record_0 = &b"launch";
                        let __seed_launch_record_1 = naclac_lang::prelude::ToAddress::address(
                            &(payer.address()),
                        );
                        let __seed_launch_record_2 = &&args.id.to_le_bytes();
                        let pinocchio_seeds = [
                            naclac_lang::prelude::pinocchio::cpi::Seed::from(
                                naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                                    &__seed_launch_record_0,
                                ),
                            ),
                            naclac_lang::prelude::pinocchio::cpi::Seed::from(
                                naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                                    &__seed_launch_record_1,
                                ),
                            ),
                            naclac_lang::prelude::pinocchio::cpi::Seed::from(
                                naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                                    &__seed_launch_record_2,
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
                        let __sys_info = &accounts[14usize];
                        if __sys_info.address()
                            != naclac_lang::prelude::SYSTEM_PROGRAM_ID
                        {
                            return Err(
                                naclac_lang::prelude::NaclacError::ProgramIdMismatch
                                    .err(14usize),
                            );
                        }
                        let __payer_info = &payer;
                        if info.data_is_empty() {
                            const __STORAGE_OVERHEAD: u64 = 128;
                            const __LAMPORTS_PER_BYTE: u64 = 6960;
                            let __space = ((8 + core::mem::size_of::<LaunchRecord>()))
                                as u64;
                            let __lamports = (__STORAGE_OVERHEAD + __space)
                                .wrapping_mul(__LAMPORTS_PER_BYTE);
                            naclac_lang::prelude::system_program::create_account_unchecked(
                                &__payer_info.view,
                                &info.view,
                                __lamports,
                                __space,
                                program_id,
                                pinocchio_signer_seeds,
                            )?;
                        }
                        unsafe {
                            let ptr = info.view.data_ptr() as *mut u8;
                            let slice = core::slice::from_raw_parts_mut(ptr, 8);
                            slice.copy_from_slice(&LaunchRecord::DISCRIMINATOR);
                        }
                    }
                }
                let mut launch_record: Account<LaunchRecord> = naclac_lang::prelude::Account::try_from(
                    info,
                    3usize,
                )?;
                let info = &__views[4usize];
                let mut launcher_token_a: Account<TokenAccount> = naclac_lang::prelude::Account::try_from_mut(
                    info,
                    4usize,
                )?;
                let info = &__views[5usize];
                let mut launcher_token_b: Account<TokenAccount> = naclac_lang::prelude::Account::try_from_mut(
                    info,
                    5usize,
                )?;
                let info = &__views[6usize];
                let mut launcher_lp: Account<TokenAccount> = naclac_lang::prelude::Account::try_from_mut(
                    info,
                    6usize,
                )?;
                let info = &__views[7usize];
                let mut payer_token_b: Account<TokenAccount> = naclac_lang::prelude::Account::try_from_mut(
                    info,
                    7usize,
                )?;
                let info = &__views[8usize];
                let mut pool_state: AccountInfo = info.clone();
                let info = &__views[9usize];
                let mut pool_vault_a: AccountInfo = info.clone();
                let info = &__views[10usize];
                let mut pool_vault_b: AccountInfo = info.clone();
                let info = &__views[11usize];
                let mut pool_lp_mint: AccountInfo = info.clone();
                let info = &__views[12usize];
                let mut amm_program: AccountInfo = info.clone();
                let info = &__views[13usize];
                let mut token_program: Program<Token> = naclac_lang::prelude::Program::try_from(
                    info,
                    &(naclac_lang::prelude::TOKEN_PROGRAM_ID),
                    13usize,
                )?;
                let info = &__views[14usize];
                let mut system_program: Program<System> = naclac_lang::prelude::Program::try_from(
                    info,
                    &(naclac_lang::prelude::SYSTEM_PROGRAM_ID),
                    14usize,
                )?;
                let __remaining: &'static [naclac_lang::prelude::AccountType] = unsafe {
                    core::mem::transmute(&__views[15usize..])
                };
                let mut __me = Self {
                    payer,
                    quote_mint,
                    mint,
                    launch_record,
                    launcher_token_a,
                    launcher_token_b,
                    launcher_lp,
                    payer_token_b,
                    pool_state,
                    pool_vault_a,
                    pool_vault_b,
                    pool_lp_mint,
                    amm_program,
                    token_program,
                    system_program,
                };
                Ok((
                    __me,
                    __remaining,
                    LaunchTokenBumps {
                        mint: __bump_mint,
                        launch_record: __bump_launch_record,
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
                bumps: &LaunchTokenBumps,
            ) -> naclac_lang::prelude::Result {
                Ok(())
            }
        }
        pub fn launch_token(ctx: Context<LaunchToken>, args: LaunchTokenArgs) -> Result {
            {
                let id_bytes = args.id.to_le_bytes();
                ctx.accounts
                    .token_program
                    .transfer(
                        &ctx.accounts.payer_token_b,
                        &ctx.accounts.launcher_token_b,
                        &ctx.accounts.payer,
                        args.amount_quote,
                    )?;
                let payer_address = ctx.accounts.payer.address();
                let record_seeds: &[&[u8]] = &[
                    b"launch",
                    payer_address.as_ref(),
                    &id_bytes,
                    &[args.launch_record_bump],
                ];
                let record_signer: &[&[&[u8]]] = &[record_seeds];
                let amount_token_total = args.amount_token_pool
                    + args.amount_token_launcher;
                ctx.accounts
                    .token_program
                    .mint_to_signed(
                        &ctx.accounts.mint,
                        &ctx.accounts.launcher_token_a,
                        &ctx.accounts.launch_record,
                        amount_token_total,
                        record_signer,
                    )?;
                let amm_cpi_accounts = amm_client::instructions::InitializeCpiAccounts {
                    payer: ctx.accounts.payer.to_account_info(),
                    token_a_mint: ctx.accounts.mint.to_account_info(),
                    token_b_mint: ctx.accounts.quote_mint.to_account_info(),
                    pool_state: ctx.accounts.pool_state.to_account_info(),
                    vault_a: ctx.accounts.pool_vault_a.to_account_info(),
                    vault_b: ctx.accounts.pool_vault_b.to_account_info(),
                    lp_mint: ctx.accounts.pool_lp_mint.to_account_info(),
                    depositor_token_a: ctx.accounts.launcher_token_a.to_account_info(),
                    depositor_token_b: ctx.accounts.launcher_token_b.to_account_info(),
                    depositor_lp: ctx.accounts.launcher_lp.to_account_info(),
                    depositor_authority: ctx.accounts.launch_record.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                };
                amm_client::instructions::InitializeCpiBuilder::new(
                        &ctx.accounts.amm_program.address(),
                        amm_cpi_accounts,
                        args.id,
                        args.pool_bump,
                        args.amount_token_pool,
                        args.amount_quote,
                    )
                    .invoke_signed(record_signer)?;
                crate::systems::process_launch_record(
                    &mut ctx.accounts.launch_record,
                    ctx.accounts.payer.address(),
                    ctx.accounts.mint.address(),
                    args.amount_token_pool,
                    args.amount_quote,
                )?;
                {
                    let mut __event = <crate::events::TokenLaunched as core::default::Default>::default();
                    __event.id = (args.id).into();
                    __event.mint = (ctx.accounts.mint.address()).into();
                    __event.amount_token = (args.amount_token_pool).into();
                    __event.amount_quote = (args.amount_quote).into();
                    __event.emit();
                };
                Ok(())
            }
        }
    }
    pub use launch_token::*;
    pub mod create_mint {
        use naclac_lang::prelude::*;
        use crate::constants::SEED_MINT;
        #[instruction(id:u64, mint_bump:u8, launch_record_bump:u8, _decimals:u8)]
        pub struct CreateMint {
            #[account(mut)]
            pub payer: Signer,
            #[account(
                init,
                payer = payer,
                seeds = [SEED_MINT,
                &id.to_le_bytes()],
                bump = mint_bump,
                mint::decimals = _decimals,
                mint::authority = launch_record,
            )]
            pub mint: AccountInfo,
            #[account(
                seeds = [b"launch",
                payer.address().as_ref(),
                &id.to_le_bytes()],
                bump = launch_record_bump
            )]
            pub launch_record: AccountInfo,
            pub token_program: Program<Token>,
            pub system_program: Program<System>,
        }
        pub struct CreateMintBumps {
            pub mint: u8,
            pub launch_record: u8,
        }
        #[automatically_derived]
        #[doc(hidden)]
        unsafe impl ::core::clone::TrivialClone for CreateMintBumps {}
        #[automatically_derived]
        impl ::core::clone::Clone for CreateMintBumps {
            #[inline]
            fn clone(&self) -> CreateMintBumps {
                let _: ::core::clone::AssertParamIsClone<u8>;
                *self
            }
        }
        #[automatically_derived]
        impl ::core::marker::Copy for CreateMintBumps {}
        #[automatically_derived]
        impl ::core::default::Default for CreateMintBumps {
            #[inline]
            fn default() -> CreateMintBumps {
                CreateMintBumps {
                    mint: ::core::default::Default::default(),
                    launch_record: ::core::default::Default::default(),
                }
            }
        }
        impl CreateMint {
            pub fn to_account_metas(
                &self,
            ) -> [naclac_lang::prelude::AccountMeta; 5usize] {
                [
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.payer).address(),
                        is_signer: true,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.mint).address(),
                        is_signer: false,
                        is_writable: true,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.launch_record).address(),
                        is_signer: false,
                        is_writable: false,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.token_program).address(),
                        is_signer: false,
                        is_writable: false,
                    },
                    naclac_lang::prelude::AccountMeta {
                        address: (&self.system_program).address(),
                        is_signer: false,
                        is_writable: false,
                    },
                ]
            }
        }
        impl CreateMint {
            pub fn to_account_infos(
                &self,
            ) -> [naclac_lang::prelude::AccountInfo; 5usize] {
                [
                    naclac_lang::prelude::ToAccountInfo::to_account_info(&self.payer),
                    naclac_lang::prelude::ToAccountInfo::to_account_info(&self.mint),
                    naclac_lang::prelude::ToAccountInfo::to_account_info(
                        &self.launch_record,
                    ),
                    naclac_lang::prelude::ToAccountInfo::to_account_info(
                        &self.token_program,
                    ),
                    naclac_lang::prelude::ToAccountInfo::to_account_info(
                        &self.system_program,
                    ),
                ]
            }
        }
        impl naclac_lang::prelude::Bumps for CreateMint {
            type BumpsStruct = CreateMintBumps;
        }
        impl CreateMint {
            /// Loads and validates the accounts provided to the instruction.
            ///
            /// ## Pinocchio path (pre-walked slice)
            /// Takes a pre-walked `&[AccountType]` slice provided by the entrypoint.
            /// Accounts were walked once globally before dispatch — O(1) slice indexing,
            /// no cursor creation, no lookup array allocation per instruction.
            ///
            /// ## Standard path (Borsh/solana-program)
            /// Takes the pre-built `&[AccountType]` slice (unchanged).
            pub fn load_and_validate(
                program_id: &naclac_lang::prelude::Address,
                __views: &[naclac_lang::prelude::AccountType],
                instruction_data: &[u8],
            ) -> Result<
                (Self, &'static [naclac_lang::prelude::AccountType], CreateMintBumps),
                naclac_lang::prelude::ProgramError,
            > {
                if __views.len() < 5usize {
                    return Err(naclac_lang::prelude::ProgramError::NotEnoughAccountKeys);
                }
                let accounts = __views;
                let mut __ix_offset: usize = 8;
                let id = {
                    let __sz = <u64 as naclac_lang::prelude::NaclacPod>::naclac_size();
                    if instruction_data.len() < __ix_offset + __sz {
                        return Err(
                            naclac_lang::prelude::NaclacError::InvalidInstructionData
                                .err(0),
                        );
                    }
                    let __val = <u64 as naclac_lang::prelude::NaclacPod>::naclac_from_bytes(
                        &instruction_data[__ix_offset..__ix_offset + __sz],
                    );
                    __ix_offset += __sz;
                    __val
                };
                let mint_bump = {
                    let __sz = <u8 as naclac_lang::prelude::NaclacPod>::naclac_size();
                    if instruction_data.len() < __ix_offset + __sz {
                        return Err(
                            naclac_lang::prelude::NaclacError::InvalidInstructionData
                                .err(0),
                        );
                    }
                    let __val = <u8 as naclac_lang::prelude::NaclacPod>::naclac_from_bytes(
                        &instruction_data[__ix_offset..__ix_offset + __sz],
                    );
                    __ix_offset += __sz;
                    __val
                };
                let launch_record_bump = {
                    let __sz = <u8 as naclac_lang::prelude::NaclacPod>::naclac_size();
                    if instruction_data.len() < __ix_offset + __sz {
                        return Err(
                            naclac_lang::prelude::NaclacError::InvalidInstructionData
                                .err(0),
                        );
                    }
                    let __val = <u8 as naclac_lang::prelude::NaclacPod>::naclac_from_bytes(
                        &instruction_data[__ix_offset..__ix_offset + __sz],
                    );
                    __ix_offset += __sz;
                    __val
                };
                let _decimals = {
                    let __sz = <u8 as naclac_lang::prelude::NaclacPod>::naclac_size();
                    if instruction_data.len() < __ix_offset + __sz {
                        return Err(
                            naclac_lang::prelude::NaclacError::InvalidInstructionData
                                .err(0),
                        );
                    }
                    let __val = <u8 as naclac_lang::prelude::NaclacPod>::naclac_from_bytes(
                        &instruction_data[__ix_offset..__ix_offset + __sz],
                    );
                    __ix_offset += __sz;
                    __val
                };
                let info = &__views[0usize];
                let mut payer: Signer = naclac_lang::prelude::Signer::try_from(
                    info,
                    0usize,
                )?;
                let info = &__views[1usize];
                let __seed_mint_0 = &SEED_MINT;
                let __seed_mint_1 = &&id.to_le_bytes();
                let expected_bump = (mint_bump);
                let __expected_bump_arr = [expected_bump];
                let __pda_program = naclac_lang::prelude::ToAddress::address(
                    &program_id,
                );
                let inputs = [
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_mint_0,
                    ),
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_mint_1,
                    ),
                    &__expected_bump_arr,
                    __pda_program.as_ref(),
                    b"ProgramDerivedAddress",
                ];
                let expected_pda = {
                    extern "C" {
                        pub fn sol_sha256(
                            vals: *const u8,
                            val_len: u64,
                            hash_result: *mut u8,
                        ) -> u32;
                    }
                    let mut hash_result = [0u8; 32];
                    unsafe {
                        sol_sha256(
                            inputs.as_ptr() as *const u8,
                            inputs.len() as u64,
                            hash_result.as_mut_ptr(),
                        );
                    }
                    naclac_lang::prelude::Address::new_from_array(hash_result)
                };
                let __key = (info).address();
                if __key != naclac_lang::prelude::ToAddress::address(&expected_pda) {
                    return Err(
                        naclac_lang::prelude::NaclacError::ConstraintSeeds.err(1usize),
                    );
                }
                if expected_bump != (mint_bump) {
                    return Err(
                        naclac_lang::prelude::NaclacError::ConstraintSeeds.err(1usize),
                    );
                }
                let __bump_mint = expected_bump;
                if !info.data_is_empty() {
                    return Err(
                        naclac_lang::prelude::NaclacError::AccountAlreadyInitialized
                            .err(1usize),
                    );
                }
                if info.data_is_empty() {
                    {
                        let __expected_bump_arr = [expected_bump];
                        let __seed_mint_0 = &SEED_MINT;
                        let __seed_mint_1 = &&id.to_le_bytes();
                        let pinocchio_seeds = [
                            naclac_lang::prelude::pinocchio::cpi::Seed::from(
                                naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                                    &__seed_mint_0,
                                ),
                            ),
                            naclac_lang::prelude::pinocchio::cpi::Seed::from(
                                naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                                    &__seed_mint_1,
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
                        let __sys_info = &accounts[4usize];
                        if __sys_info.address()
                            != naclac_lang::prelude::SYSTEM_PROGRAM_ID
                        {
                            return Err(
                                naclac_lang::prelude::NaclacError::ProgramIdMismatch
                                    .err(4usize),
                            );
                        }
                        let __tok_prog_info = &accounts[3usize];
                        let __tok_prog_key = __tok_prog_info.address();
                        if __tok_prog_key != naclac_lang::prelude::TOKEN_PROGRAM_ID
                            && __tok_prog_key
                                != naclac_lang::prelude::TOKEN_2022_PROGRAM_ID
                        {
                            return Err(
                                naclac_lang::prelude::NaclacError::ProgramIdMismatch
                                    .err(3usize),
                            );
                        }
                        let __payer_view = naclac_lang::prelude::ToAccountInfo::to_account_info(
                                &payer,
                            )
                            .view;
                        let __lamports = {
                            const __STORAGE_OVERHEAD: u64 = 128;
                            const __LAMPORTS_PER_BYTE: u64 = 6960;
                            (__STORAGE_OVERHEAD + 82u64)
                                .wrapping_mul(__LAMPORTS_PER_BYTE)
                        };
                        naclac_lang::prelude::system_program::create_account_unchecked(
                            &__payer_view,
                            &info.view,
                            __lamports,
                            82,
                            &__tok_prog_key,
                            pinocchio_signer_seeds,
                        )?;
                        let __authority_info = naclac_lang::prelude::ToAccountInfo::to_account_info(
                            &accounts[2usize],
                        );
                        let __authority_key = (&__authority_info).address();
                        let __freeze_key_view = None;
                        let __init_ix = naclac_lang::prelude::pinocchio_token_2022::instructions::InitializeMint2 {
                            token_program: __tok_prog_key.as_address(),
                            mint: &info.view,
                            decimals: _decimals,
                            mint_authority: __authority_key.as_address(),
                            freeze_authority: __freeze_key_view,
                        };
                        __init_ix.invoke()?;
                    }
                }
                let mut mint: AccountInfo = info.clone();
                let info = &__views[2usize];
                let mut launch_record: AccountInfo = info.clone();
                let __seed_launch_record_0 = &b"launch";
                let __seed_launch_record_1 = naclac_lang::prelude::ToAddress::address(
                    &(payer.address()),
                );
                let __seed_launch_record_2 = &&id.to_le_bytes();
                let expected_bump = (launch_record_bump);
                let __expected_bump_arr = [expected_bump];
                let __pda_program = naclac_lang::prelude::ToAddress::address(
                    &program_id,
                );
                let inputs = [
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_launch_record_0,
                    ),
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_launch_record_1,
                    ),
                    naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(
                        &__seed_launch_record_2,
                    ),
                    &__expected_bump_arr,
                    __pda_program.as_ref(),
                    b"ProgramDerivedAddress",
                ];
                let expected_pda = {
                    extern "C" {
                        pub fn sol_sha256(
                            vals: *const u8,
                            val_len: u64,
                            hash_result: *mut u8,
                        ) -> u32;
                    }
                    let mut hash_result = [0u8; 32];
                    unsafe {
                        sol_sha256(
                            inputs.as_ptr() as *const u8,
                            inputs.len() as u64,
                            hash_result.as_mut_ptr(),
                        );
                    }
                    naclac_lang::prelude::Address::new_from_array(hash_result)
                };
                if (launch_record).address()
                    != naclac_lang::prelude::ToAddress::address(&expected_pda)
                {
                    return Err(
                        naclac_lang::prelude::NaclacError::ConstraintSeeds.err(2usize),
                    );
                }
                if expected_bump != (launch_record_bump) {
                    return Err(
                        naclac_lang::prelude::NaclacError::ConstraintSeeds.err(2usize),
                    );
                }
                let __bump_launch_record = expected_bump;
                let info = &__views[3usize];
                let mut token_program: Program<Token> = naclac_lang::prelude::Program::try_from(
                    info,
                    &(naclac_lang::prelude::TOKEN_PROGRAM_ID),
                    3usize,
                )?;
                let info = &__views[4usize];
                let mut system_program: Program<System> = naclac_lang::prelude::Program::try_from(
                    info,
                    &(naclac_lang::prelude::SYSTEM_PROGRAM_ID),
                    4usize,
                )?;
                let __remaining: &'static [naclac_lang::prelude::AccountType] = unsafe {
                    core::mem::transmute(&__views[5usize..])
                };
                let mut __me = Self {
                    payer,
                    mint,
                    launch_record,
                    token_program,
                    system_program,
                };
                Ok((
                    __me,
                    __remaining,
                    CreateMintBumps {
                        mint: __bump_mint,
                        launch_record: __bump_launch_record,
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
                bumps: &CreateMintBumps,
            ) -> naclac_lang::prelude::Result {
                Ok(())
            }
        }
        pub fn create_mint(
            ctx: Context<CreateMint>,
            id: u64,
            _mint_bump: u8,
            _launch_record_bump: u8,
            decimals: u8,
        ) -> Result {
            {
                {
                    let mut __event = <crate::events::MintCreated as core::default::Default>::default();
                    __event.id = (id).into();
                    __event.mint = (ctx.accounts.mint.address()).into();
                    __event.decimals = (decimals).into();
                    __event.emit();
                };
                Ok(())
            }
        }
    }
    pub use create_mint::*;
}
pub mod events {
    use naclac_lang::prelude::*;
    #[bytemuck(crate = "naclac_lang::bytemuck")]
    #[repr(C)]
    pub struct TokenLaunched {
        pub id: u64,
        pub mint: Address,
        pub amount_token: u64,
        pub amount_quote: u64,
    }
    #[automatically_derived]
    #[doc(hidden)]
    unsafe impl ::core::clone::TrivialClone for TokenLaunched {}
    #[automatically_derived]
    impl ::core::clone::Clone for TokenLaunched {
        #[inline]
        fn clone(&self) -> TokenLaunched {
            let _: ::core::clone::AssertParamIsClone<u64>;
            let _: ::core::clone::AssertParamIsClone<Address>;
            *self
        }
    }
    #[automatically_derived]
    impl ::core::marker::Copy for TokenLaunched {}
    #[automatically_derived]
    impl ::core::default::Default for TokenLaunched {
        #[inline]
        fn default() -> TokenLaunched {
            TokenLaunched {
                id: ::core::default::Default::default(),
                mint: ::core::default::Default::default(),
                amount_token: ::core::default::Default::default(),
                amount_quote: ::core::default::Default::default(),
            }
        }
    }
    const _: () = {
        if !(::core::mem::size_of::<TokenLaunched>()
            == (::core::mem::size_of::<u64>() + ::core::mem::size_of::<Address>()
                + ::core::mem::size_of::<u64>() + ::core::mem::size_of::<u64>()))
        {
            ::core::panicking::panic("derive(Pod) was applied to a type with padding")
        }
    };
    const _: fn() = || {
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Pod>() {}
            assert_impl::<u64>();
        }
    };
    const _: fn() = || {
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Pod>() {}
            assert_impl::<Address>();
        }
    };
    const _: fn() = || {
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Pod>() {}
            assert_impl::<u64>();
        }
    };
    const _: fn() = || {
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Pod>() {}
            assert_impl::<u64>();
        }
    };
    unsafe impl naclac_lang::bytemuck::Pod for TokenLaunched {}
    const _: fn() = || {
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Zeroable>() {}
            assert_impl::<u64>();
        }
    };
    const _: fn() = || {
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Zeroable>() {}
            assert_impl::<Address>();
        }
    };
    const _: fn() = || {
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Zeroable>() {}
            assert_impl::<u64>();
        }
    };
    const _: fn() = || {
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Zeroable>() {}
            assert_impl::<u64>();
        }
    };
    unsafe impl naclac_lang::bytemuck::Zeroable for TokenLaunched {}
    impl TokenLaunched {
        pub fn emit(&self) {
            {
                let discriminator = [
                    225u8, 232u8, 190u8, 147u8, 213u8, 192u8, 220u8, 168u8,
                ];
                let data = naclac_lang::prelude::bytemuck::bytes_of(self);
                naclac_lang::prelude::sol_log_data(&[&discriminator, data]);
            }
        }
    }
    #[bytemuck(crate = "naclac_lang::bytemuck")]
    #[repr(C)]
    pub struct MintCreated {
        pub id: u64,
        pub mint: Address,
        pub decimals: u8,
        pub _padding: [u8; 7usize],
    }
    #[automatically_derived]
    #[doc(hidden)]
    unsafe impl ::core::clone::TrivialClone for MintCreated {}
    #[automatically_derived]
    impl ::core::clone::Clone for MintCreated {
        #[inline]
        fn clone(&self) -> MintCreated {
            let _: ::core::clone::AssertParamIsClone<u64>;
            let _: ::core::clone::AssertParamIsClone<Address>;
            let _: ::core::clone::AssertParamIsClone<u8>;
            let _: ::core::clone::AssertParamIsClone<[u8; 7usize]>;
            *self
        }
    }
    #[automatically_derived]
    impl ::core::marker::Copy for MintCreated {}
    #[automatically_derived]
    impl ::core::default::Default for MintCreated {
        #[inline]
        fn default() -> MintCreated {
            MintCreated {
                id: ::core::default::Default::default(),
                mint: ::core::default::Default::default(),
                decimals: ::core::default::Default::default(),
                _padding: ::core::default::Default::default(),
            }
        }
    }
    const _: () = {
        if !(::core::mem::size_of::<MintCreated>()
            == (::core::mem::size_of::<u64>() + ::core::mem::size_of::<Address>()
                + ::core::mem::size_of::<u8>() + ::core::mem::size_of::<[u8; 7usize]>()))
        {
            ::core::panicking::panic("derive(Pod) was applied to a type with padding")
        }
    };
    const _: fn() = || {
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Pod>() {}
            assert_impl::<u64>();
        }
    };
    const _: fn() = || {
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Pod>() {}
            assert_impl::<Address>();
        }
    };
    const _: fn() = || {
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Pod>() {}
            assert_impl::<u8>();
        }
    };
    const _: fn() = || {
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Pod>() {}
            assert_impl::<[u8; 7usize]>();
        }
    };
    unsafe impl naclac_lang::bytemuck::Pod for MintCreated {}
    const _: fn() = || {
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Zeroable>() {}
            assert_impl::<u64>();
        }
    };
    const _: fn() = || {
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Zeroable>() {}
            assert_impl::<Address>();
        }
    };
    const _: fn() = || {
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Zeroable>() {}
            assert_impl::<u8>();
        }
    };
    const _: fn() = || {
        #[doc(hidden)]
        fn check() {
            fn assert_impl<T: naclac_lang::bytemuck::Zeroable>() {}
            assert_impl::<[u8; 7usize]>();
        }
    };
    unsafe impl naclac_lang::bytemuck::Zeroable for MintCreated {}
    impl MintCreated {
        pub fn emit(&self) {
            {
                let discriminator = [
                    254u8, 157u8, 196u8, 76u8, 231u8, 48u8, 27u8, 150u8,
                ];
                let data = naclac_lang::prelude::bytemuck::bytes_of(self);
                naclac_lang::prelude::sol_log_data(&[&discriminator, data]);
            }
        }
    }
}
pub mod errors {
    use naclac_lang::prelude::*;
    pub enum LaunchpadError {
        /// Invalid program ID.
        InvalidProgramId,
        /// Unauthorized action.
        Unauthorized,
        /// Overflow or math error.
        Overflow,
        /// Zero amount provided.
        ZeroAmount,
        /// Mint failed.
        MintFailed,
    }
    #[automatically_derived]
    impl ::core::marker::Copy for LaunchpadError {}
    #[automatically_derived]
    #[doc(hidden)]
    unsafe impl ::core::clone::TrivialClone for LaunchpadError {}
    #[automatically_derived]
    impl ::core::clone::Clone for LaunchpadError {
        #[inline]
        fn clone(&self) -> LaunchpadError {
            *self
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for LaunchpadError {
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_fields_are_eq(&self) {}
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for LaunchpadError {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for LaunchpadError {
        #[inline]
        fn eq(&self, other: &LaunchpadError) -> bool {
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            let __arg1_discr = ::core::intrinsics::discriminant_value(other);
            __self_discr == __arg1_discr
        }
    }
    impl From<LaunchpadError> for naclac_lang::prelude::ProgramError {
        fn from(e: LaunchpadError) -> Self {
            naclac_lang::prelude::ProgramError::Custom(e as u32 + 6000)
        }
    }
}
pub mod constants {
    pub const SEED_MINT: &[u8] = b"mint";
}
pub mod systems {
    pub mod launch {
        use naclac_lang::prelude::*;
        use crate::components::launch_record::LaunchRecord;
        pub fn process_launch_record(
            launch_record: &mut LaunchRecord,
            creator: Address,
            mint: Address,
            amount_token: u64,
            amount_quote: u64,
        ) -> Result<()> {
            let __result = {
                {
                    launch_record.creator = creator;
                    launch_record.mint = mint;
                    launch_record.amount_token = amount_token;
                    launch_record.amount_quote = amount_quote;
                    Ok(())
                }
            };
            __result
        }
    }
    pub use launch::*;
}
use instructions::*;
#[no_mangle]
pub unsafe extern "C" fn entrypoint(input: *mut u8, ix_data_ptr: *const u8) -> u64 {
    match process_instruction(input, ix_data_ptr) {
        Ok(()) => naclac_lang::pinocchio::SUCCESS,
        Err(e) => e.into(),
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
    if instruction_data.len() < 8 {
        return Err(
            naclac_lang::prelude::ProgramError::Custom(
                naclac_lang::prelude::NaclacError::InvalidInstructionData as u32,
            ),
        );
    }
    let instruction_discriminator = u64::from_le_bytes(unsafe {
        *(instruction_data.as_ptr() as *const [u8; 8])
    });
    let mut __lookup = [core::mem::MaybeUninit::<
        naclac_lang::prelude::AccountView,
    >::uninit(); 64];
    let __views: &'static [naclac_lang::prelude::AccountType] = unsafe {
        let mut __cursor = naclac_lang::prelude::AccountCursor::new(
            input,
            __lookup.as_mut_ptr() as *mut naclac_lang::prelude::AccountView,
        );
        let raw = __cursor.walk_n(num_accounts);
        core::mem::transmute(raw)
    };
    match instruction_discriminator {
        17627521065357312010u64 => {
            token_creator::__dispatch_launch_token(program_id, __views, instruction_data)
        }
        3254368590095658053u64 => {
            token_creator::__dispatch_create_mint(program_id, __views, instruction_data)
        }
        _ => {
            Err(
                naclac_lang::prelude::ProgramError::Custom(
                    naclac_lang::prelude::NaclacError::InvalidInstructionData as u32,
                ),
            )
        }
    }
}
pub mod token_creator {
    use super::*;
    pub(crate) fn __dispatch_launch_token(
        program_id: &'static naclac_lang::prelude::Address,
        __views: &'static [naclac_lang::prelude::AccountType],
        instruction_data: &'static [u8],
    ) -> naclac_lang::prelude::ProgramResult {
        let mut __ix_offset: usize = 8;
        let args = {
            let __sz = <LaunchTokenArgs as naclac_lang::prelude::NaclacPod>::naclac_size();
            if instruction_data.len() < __ix_offset + __sz {
                return Err(
                    naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0),
                );
            }
            let __val = <LaunchTokenArgs as naclac_lang::prelude::NaclacPod>::naclac_from_bytes(
                &instruction_data[__ix_offset..__ix_offset + __sz],
            );
            __ix_offset += __sz;
            __val
        };
        let (mut accounts, __rem_slice, bumps) = LaunchToken::load_and_validate(
            program_id,
            __views,
            instruction_data,
        )?;
        let ctx = unsafe {
            naclac_lang::prelude::Context::new(
                program_id,
                &mut accounts,
                __rem_slice,
                bumps,
            )
        };
        let __result = token_creator::launch_token(ctx, args);
        if __result.is_ok() {
            accounts.teardown(program_id, &bumps)?;
        }
        __result
    }
    pub fn launch_token(ctx: Context<LaunchToken>, args: LaunchTokenArgs) -> Result {
        launch_token::launch_token(ctx, args)
    }
    pub(crate) fn __dispatch_create_mint(
        program_id: &'static naclac_lang::prelude::Address,
        __views: &'static [naclac_lang::prelude::AccountType],
        instruction_data: &'static [u8],
    ) -> naclac_lang::prelude::ProgramResult {
        let mut __ix_offset: usize = 8;
        let id = {
            let __sz = <u64 as naclac_lang::prelude::NaclacPod>::naclac_size();
            if instruction_data.len() < __ix_offset + __sz {
                return Err(
                    naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0),
                );
            }
            let __val = <u64 as naclac_lang::prelude::NaclacPod>::naclac_from_bytes(
                &instruction_data[__ix_offset..__ix_offset + __sz],
            );
            __ix_offset += __sz;
            __val
        };
        let mint_bump = {
            let __sz = <u8 as naclac_lang::prelude::NaclacPod>::naclac_size();
            if instruction_data.len() < __ix_offset + __sz {
                return Err(
                    naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0),
                );
            }
            let __val = <u8 as naclac_lang::prelude::NaclacPod>::naclac_from_bytes(
                &instruction_data[__ix_offset..__ix_offset + __sz],
            );
            __ix_offset += __sz;
            __val
        };
        let launch_record_bump = {
            let __sz = <u8 as naclac_lang::prelude::NaclacPod>::naclac_size();
            if instruction_data.len() < __ix_offset + __sz {
                return Err(
                    naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0),
                );
            }
            let __val = <u8 as naclac_lang::prelude::NaclacPod>::naclac_from_bytes(
                &instruction_data[__ix_offset..__ix_offset + __sz],
            );
            __ix_offset += __sz;
            __val
        };
        let decimals = {
            let __sz = <u8 as naclac_lang::prelude::NaclacPod>::naclac_size();
            if instruction_data.len() < __ix_offset + __sz {
                return Err(
                    naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0),
                );
            }
            let __val = <u8 as naclac_lang::prelude::NaclacPod>::naclac_from_bytes(
                &instruction_data[__ix_offset..__ix_offset + __sz],
            );
            __ix_offset += __sz;
            __val
        };
        let (mut accounts, __rem_slice, bumps) = CreateMint::load_and_validate(
            program_id,
            __views,
            instruction_data,
        )?;
        let ctx = unsafe {
            naclac_lang::prelude::Context::new(
                program_id,
                &mut accounts,
                __rem_slice,
                bumps,
            )
        };
        let __result = token_creator::create_mint(
            ctx,
            id,
            mint_bump,
            launch_record_bump,
            decimals,
        );
        if __result.is_ok() {
            accounts.teardown(program_id, &bumps)?;
        }
        __result
    }
    pub fn create_mint(
        ctx: Context<CreateMint>,
        id: u64,
        mint_bump: u8,
        launch_record_bump: u8,
        decimals: u8,
    ) -> Result {
        create_mint::create_mint(ctx, id, mint_bump, launch_record_bump, decimals)
    }
}
