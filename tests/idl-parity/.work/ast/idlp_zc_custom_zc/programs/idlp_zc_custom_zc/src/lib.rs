//! Same program as the `zc` tree with a custom 2-byte discriminator on every handler.

use naclac_lang::prelude::*;

declare_id!("maSDvF6t8ujNtgZNnhwQjY48YKTKH6oq73V2vtJ2pkf");

pub mod components;
pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod types;

use instructions::*;
use types::{Level, Mode, Nested, Pair, Shape, Signed, Wrapper};

#[program]
pub mod idlp_zc_custom_zc {
    /// Documented on the wrapper and, differently, on the implementation: the wrapper wins.
    #[discriminator = [170, 1]]
    pub fn wrapper_kinds(ctx: Context<WrapperKinds>) -> Result {
        basics::wrapper_kinds(ctx)
    }

    #[discriminator = [170, 2]]
    pub fn constraint_flags(ctx: Context<ConstraintFlags>) -> Result {
        flags::constraint_flags(ctx)
    }

    #[discriminator = [170, 3]]
    pub fn relations(ctx: Context<Relations>) -> Result {
        relations::relations(ctx)
    }

    #[discriminator = [170, 4]]
    pub fn init_plain(ctx: Context<InitPlain>) -> Result {
        inits::init_plain(ctx)
    }

    #[discriminator = [170, 5]]
    pub fn init_if_needed(ctx: Context<InitIfNeeded>) -> Result {
        inits::init_if_needed(ctx)
    }

    #[discriminator = [170, 6]]
    pub fn init_unchecked(ctx: Context<InitUnchecked>) -> Result {
        inits::init_unchecked(ctx)
    }

    #[discriminator = [170, 7]]
    pub fn close_it(ctx: Context<CloseIt>) -> Result {
        inits::close_it(ctx)
    }

    #[discriminator = [170, 8]]
    pub fn implicit_mut(ctx: Context<ImplicitMut>) -> Result {
        inits::implicit_mut(ctx)
    }

    #[discriminator = [170, 9]]
    pub fn seed_forms_literal(ctx: Context<SeedFormsLiteral>) -> Result {
        seeds::seed_forms_literal(ctx)
    }

    #[discriminator = [170, 10]]
    pub fn seed_forms_dynamic(
        ctx: Context<SeedFormsDynamic>,
        id: u64,
        platform: u8,
        label: [u8; 8],
        bump: u8,
    ) -> Result {
        seeds::seed_forms_dynamic(ctx, id, platform, label, bump)
    }

    #[discriminator = [170, 11]]
    pub fn create_mint(ctx: Context<CreateMint>, id: u64, mint_bump: u8, decimals: u8) -> Result {
        tokens::create_mint(ctx, id, mint_bump, decimals)
    }

    #[discriminator = [170, 12]]
    pub fn create_mint_2022(ctx: Context<CreateMint2022>) -> Result {
        tokens::create_mint_2022(ctx)
    }

    #[discriminator = [170, 13]]
    pub fn create_ata(ctx: Context<CreateAta>) -> Result {
        tokens::create_ata(ctx)
    }

    #[discriminator = [170, 14]]
    pub fn check_token_program(ctx: Context<CheckTokenProgram>) -> Result {
        tokens::check_token_program(ctx)
    }

    #[discriminator = [170, 15]]
    pub fn typed_token_accounts(ctx: Context<TypedTokenAccounts>, ata_bump: u8) -> Result {
        tokens::typed_token_accounts(ctx, ata_bump)
    }

    #[discriminator = [170, 16]]
    pub fn realloc_expr(ctx: Context<ReallocExpr>, new_space: u64) -> Result {
        realloc_family::realloc_expr(ctx, new_space)
    }

    #[discriminator = [170, 17]]
    pub fn realloc_any_of(ctx: Context<ReallocAnyOf>) -> Result {
        realloc_family::realloc_any_of(ctx)
    }

    #[discriminator = [170, 18]]
    pub fn optional_kinds(ctx: Context<OptionalKinds>) -> Result {
        optional::optional_kinds(ctx)
    }

    /// Unsigned integer arguments.
    #[discriminator = [170, 19]]
    pub fn unsigned_args(ctx: Context<Quiet>, a: u8, b: u16, c: u32, _d: u64) -> Result {
        args::unsigned_args(ctx, a, b, c, _d)
    }

    #[discriminator = [170, 20]]
    pub fn signed_args(ctx: Context<Quiet>, e: i8, f: i16, g: i32, h: i64) -> Result {
        args::signed_args(ctx, e, f, g, h)
    }

    #[discriminator = [170, 21]]
    pub fn bool_arg(ctx: Context<Quiet>, flag: Bool) -> Result {
        args::bool_arg(ctx, flag)
    }

    #[discriminator = [170, 22]]
    pub fn composites(ctx: Context<Quiet>, who: Address, bytes: [u8; 32]) -> Result {
        args::composites(ctx, who, bytes)
    }

    #[discriminator = [170, 23]]
    pub fn zero_copy_views(
        ctx: Context<Quiet>,
        bytes: ZcVec<u8>,
        pairs: ZcVec<Pair>,
        text: ZcString,
    ) -> Result {
        args::zero_copy_views(ctx, bytes, pairs, text)
    }

    #[discriminator = [170, 24]]
    pub fn heap_args(
        ctx: Context<Quiet>,
        #[allow_heap] bytes: Vec<u8>,
        #[allow_heap] text: String,
        #[allow_heap] pairs: Vec<Pair>,
    ) -> Result {
        args::heap_args(ctx, bytes, text, pairs)
    }

    #[discriminator = [170, 25]]
    pub fn defined_struct_args(
        ctx: Context<Quiet>,
        pair: Pair,
        wrapper: Wrapper,
        nested: Nested,
    ) -> Result {
        args::defined_struct_args(ctx, pair, wrapper, nested)
    }

    #[discriminator = [170, 26]]
    pub fn defined_enum_args(
        ctx: Context<Quiet>,
        mode: Mode,
        level: Level,
        signed: Signed,
        shape: Shape,
    ) -> Result {
        args::defined_enum_args(ctx, mode, level, signed, shape)
    }

    #[discriminator = [170, 27]]
    pub fn bundled(ctx: Context<Quiet>, args: Bundle) -> Result {
        args::bundled(ctx, args)
    }

    #[discriminator = [170, 28]]
    pub fn orphan_unused(ctx: Context<Quiet>) -> Result {
        args::orphan_unused(ctx)
    }

    #[discriminator = [170, 29]]
    pub fn returns_value(ctx: Context<Returning>) -> Result<u64> {
        returns::returns_value(ctx)
    }

    #[discriminator = [170, 30]]
    pub fn returns_optional(ctx: Context<Returning>) -> Result<Option<u64>> {
        returns::returns_optional(ctx)
    }

    #[discriminator = [170, 31]]
    pub fn emits(ctx: Context<Returning>) -> Result {
        returns::emits(ctx)
    }

    #[discriminator = [170, 32]]
    pub fn cpi_emit(ctx: Context<CpiEmit>) -> Result {
        cpi_events::cpi_emit(ctx)
    }
}
