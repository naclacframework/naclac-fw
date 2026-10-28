use naclac_lang::prelude::*;

declare_id!("B4bdSZqYPru86huFYLYbP1tM8vRsHarYpZond89rwh96");

pub mod components;
pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod types;

use instructions::*;
use types::{Level, Mode, Nested, Pair, Shape, Signed, Wrapper};

#[program]
pub mod idlp_serde_custom_borsh {
    /// Documented on the wrapper and, differently, on the implementation: the wrapper wins.
    #[discriminator = 0]
    pub fn wrapper_kinds(ctx: Context<WrapperKinds>) -> Result {
        basics::wrapper_kinds(ctx)
    }

    #[discriminator = 1]
    pub fn constraint_flags(ctx: Context<ConstraintFlags>) -> Result {
        flags::constraint_flags(ctx)
    }

    #[discriminator = 2]
    pub fn relations(ctx: Context<Relations>) -> Result {
        relations::relations(ctx)
    }

    #[discriminator = 3]
    pub fn init_plain(ctx: Context<InitPlain>) -> Result {
        inits::init_plain(ctx)
    }

    #[discriminator = 4]
    pub fn init_if_needed(ctx: Context<InitIfNeeded>) -> Result {
        inits::init_if_needed(ctx)
    }

    #[discriminator = 5]
    pub fn init_unchecked(ctx: Context<InitUnchecked>) -> Result {
        inits::init_unchecked(ctx)
    }

    #[discriminator = 6]
    pub fn close_it(ctx: Context<CloseIt>) -> Result {
        inits::close_it(ctx)
    }

    #[discriminator = 7]
    pub fn implicit_mut(ctx: Context<ImplicitMut>) -> Result {
        inits::implicit_mut(ctx)
    }

    #[discriminator = 8]
    pub fn seed_forms_literal(ctx: Context<SeedFormsLiteral>) -> Result {
        seeds::seed_forms_literal(ctx)
    }

    #[discriminator = 9]
    pub fn seed_forms_dynamic(
        ctx: Context<SeedFormsDynamic>,
        id: u64,
        platform: u8,
        label: [u8; 8],
        bump: u8,
    ) -> Result {
        seeds::seed_forms_dynamic(ctx, id, platform, label, bump)
    }

    #[discriminator = 10]
    pub fn create_mint(ctx: Context<CreateMint>, id: u64, mint_bump: u8, decimals: u8) -> Result {
        tokens::create_mint(ctx, id, mint_bump, decimals)
    }

    #[discriminator = 11]
    pub fn create_mint_2022(ctx: Context<CreateMint2022>) -> Result {
        tokens::create_mint_2022(ctx)
    }

    #[discriminator = 12]
    pub fn create_ata(ctx: Context<CreateAta>) -> Result {
        tokens::create_ata(ctx)
    }

    #[discriminator = 13]
    pub fn check_token_program(ctx: Context<CheckTokenProgram>) -> Result {
        tokens::check_token_program(ctx)
    }

    #[discriminator = 14]
    pub fn boxed(ctx: Context<Boxed>) -> Result {
        boxed::boxed(ctx)
    }

    #[discriminator = 15]
    pub fn typed_token_accounts(ctx: Context<TypedTokenAccounts>, ata_bump: u8) -> Result {
        tokens::typed_token_accounts(ctx, ata_bump)
    }

    #[discriminator = 16]
    pub fn realloc_expr(ctx: Context<ReallocExpr>, new_space: u64) -> Result {
        realloc_family::realloc_expr(ctx, new_space)
    }

    #[discriminator = 17]
    pub fn realloc_any_of(ctx: Context<ReallocAnyOf>) -> Result {
        realloc_family::realloc_any_of(ctx)
    }

    #[discriminator = 18]
    pub fn optional_kinds(ctx: Context<OptionalKinds>) -> Result {
        optional::optional_kinds(ctx)
    }

    /// Unsigned integer arguments.
    #[discriminator = 19]
    pub fn unsigned_args(ctx: Context<Quiet>, a: u8, b: u16, c: u32, _d: u64) -> Result {
        args::unsigned_args(ctx, a, b, c, _d)
    }

    #[discriminator = 20]
    pub fn signed_args(ctx: Context<Quiet>, e: i8, f: i16, g: i32, h: i64) -> Result {
        args::signed_args(ctx, e, f, g, h)
    }

    #[discriminator = 21]
    pub fn bool_arg(ctx: Context<Quiet>, flag: bool) -> Result {
        args::bool_arg(ctx, flag)
    }

    #[discriminator = 22]
    pub fn composites(ctx: Context<Quiet>, who: Address, bytes: [u8; 32]) -> Result {
        args::composites(ctx, who, bytes)
    }

    #[discriminator = 23]
    pub fn wide_and_optional_args(
        ctx: Context<Quiet>,
        #[allow_heap] huge: u128,
        #[allow_heap] huge_signed: i128,
        #[allow_heap] maybe: Option<u64>,
        #[allow_heap] maybe_pair: Option<Pair>,
        #[allow_heap] maybe_vec: Option<Vec<u8>>,
    ) -> Result {
        args::wide_and_optional_args(ctx, huge, huge_signed, maybe, maybe_pair, maybe_vec)
    }

    #[discriminator = 24]
    pub fn heap_args(
        ctx: Context<Quiet>,
        #[allow_heap] bytes: Vec<u8>,
        #[allow_heap] text: String,
        #[allow_heap] pairs: Vec<Pair>,
    ) -> Result {
        args::heap_args(ctx, bytes, text, pairs)
    }

    #[discriminator = 25]
    pub fn defined_struct_args(
        ctx: Context<Quiet>,
        pair: Pair,
        wrapper: Wrapper,
        nested: Nested,
    ) -> Result {
        args::defined_struct_args(ctx, pair, wrapper, nested)
    }

    #[discriminator = 26]
    pub fn defined_enum_args(
        ctx: Context<Quiet>,
        mode: Mode,
        level: Level,
        signed: Signed,
        shape: Shape,
    ) -> Result {
        args::defined_enum_args(ctx, mode, level, signed, shape)
    }

    #[discriminator = 27]
    pub fn bundled(ctx: Context<Quiet>, args: Bundle) -> Result {
        args::bundled(ctx, args)
    }

    #[discriminator = 28]
    pub fn orphan_unused(ctx: Context<Quiet>) -> Result {
        args::orphan_unused(ctx)
    }

    #[discriminator = 29]
    pub fn returns_value(ctx: Context<Returning>) -> Result<u64> {
        returns::returns_value(ctx)
    }

    #[discriminator = 30]
    pub fn returns_optional(ctx: Context<Returning>) -> Result<Option<u64>> {
        returns::returns_optional(ctx)
    }

    #[discriminator = 31]
    pub fn emits(ctx: Context<Returning>) -> Result {
        returns::emits(ctx)
    }

    #[discriminator = 32]
    pub fn cpi_emit(ctx: Context<CpiEmit>) -> Result {
        cpi_events::cpi_emit(ctx)
    }
}
