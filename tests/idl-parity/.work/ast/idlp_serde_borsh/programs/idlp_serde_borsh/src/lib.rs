use naclac_lang::prelude::*;

declare_id!("7AAfzYbZWuqkRKtKCV1rj3P6jpxeTLaN39YHXmKvHk7C");

pub mod components;
pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod types;

use instructions::*;
use types::{Level, Mode, Nested, Pair, Shape, Signed, Wrapper};

#[program]
pub mod idlp_serde_borsh {
    /// Documented on the wrapper and, differently, on the implementation: the wrapper wins.
    pub fn wrapper_kinds(ctx: Context<WrapperKinds>) -> Result {
        basics::wrapper_kinds(ctx)
    }

    pub fn constraint_flags(ctx: Context<ConstraintFlags>) -> Result {
        flags::constraint_flags(ctx)
    }

    pub fn relations(ctx: Context<Relations>) -> Result {
        relations::relations(ctx)
    }

    pub fn init_plain(ctx: Context<InitPlain>) -> Result {
        inits::init_plain(ctx)
    }

    pub fn init_if_needed(ctx: Context<InitIfNeeded>) -> Result {
        inits::init_if_needed(ctx)
    }

    pub fn init_unchecked(ctx: Context<InitUnchecked>) -> Result {
        inits::init_unchecked(ctx)
    }

    pub fn close_it(ctx: Context<CloseIt>) -> Result {
        inits::close_it(ctx)
    }

    pub fn implicit_mut(ctx: Context<ImplicitMut>) -> Result {
        inits::implicit_mut(ctx)
    }

    pub fn seed_forms_literal(ctx: Context<SeedFormsLiteral>) -> Result {
        seeds::seed_forms_literal(ctx)
    }

    pub fn seed_forms_dynamic(
        ctx: Context<SeedFormsDynamic>,
        id: u64,
        platform: u8,
        label: [u8; 8],
        bump: u8,
    ) -> Result {
        seeds::seed_forms_dynamic(ctx, id, platform, label, bump)
    }

    pub fn create_mint(ctx: Context<CreateMint>, id: u64, mint_bump: u8, decimals: u8) -> Result {
        tokens::create_mint(ctx, id, mint_bump, decimals)
    }

    pub fn create_mint_2022(ctx: Context<CreateMint2022>) -> Result {
        tokens::create_mint_2022(ctx)
    }

    pub fn create_ata(ctx: Context<CreateAta>) -> Result {
        tokens::create_ata(ctx)
    }

    pub fn check_token_program(ctx: Context<CheckTokenProgram>) -> Result {
        tokens::check_token_program(ctx)
    }

    pub fn boxed(ctx: Context<Boxed>) -> Result {
        boxed::boxed(ctx)
    }

    pub fn typed_token_accounts(ctx: Context<TypedTokenAccounts>, ata_bump: u8) -> Result {
        tokens::typed_token_accounts(ctx, ata_bump)
    }

    pub fn realloc_expr(ctx: Context<ReallocExpr>, new_space: u64) -> Result {
        realloc_family::realloc_expr(ctx, new_space)
    }

    pub fn realloc_any_of(ctx: Context<ReallocAnyOf>) -> Result {
        realloc_family::realloc_any_of(ctx)
    }

    pub fn optional_kinds(ctx: Context<OptionalKinds>) -> Result {
        optional::optional_kinds(ctx)
    }

    /// Unsigned integer arguments.
    pub fn unsigned_args(ctx: Context<Quiet>, a: u8, b: u16, c: u32, _d: u64) -> Result {
        args::unsigned_args(ctx, a, b, c, _d)
    }

    pub fn signed_args(ctx: Context<Quiet>, e: i8, f: i16, g: i32, h: i64) -> Result {
        args::signed_args(ctx, e, f, g, h)
    }

    pub fn bool_arg(ctx: Context<Quiet>, flag: bool) -> Result {
        args::bool_arg(ctx, flag)
    }

    pub fn composites(ctx: Context<Quiet>, who: Address, bytes: [u8; 32]) -> Result {
        args::composites(ctx, who, bytes)
    }

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

    pub fn heap_args(
        ctx: Context<Quiet>,
        #[allow_heap] bytes: Vec<u8>,
        #[allow_heap] text: String,
        #[allow_heap] pairs: Vec<Pair>,
    ) -> Result {
        args::heap_args(ctx, bytes, text, pairs)
    }

    pub fn defined_struct_args(
        ctx: Context<Quiet>,
        pair: Pair,
        wrapper: Wrapper,
        nested: Nested,
    ) -> Result {
        args::defined_struct_args(ctx, pair, wrapper, nested)
    }

    pub fn defined_enum_args(
        ctx: Context<Quiet>,
        mode: Mode,
        level: Level,
        signed: Signed,
        shape: Shape,
    ) -> Result {
        args::defined_enum_args(ctx, mode, level, signed, shape)
    }

    pub fn bundled(ctx: Context<Quiet>, args: Bundle) -> Result {
        args::bundled(ctx, args)
    }

    pub fn orphan_unused(ctx: Context<Quiet>) -> Result {
        args::orphan_unused(ctx)
    }

    pub fn returns_value(ctx: Context<Returning>) -> Result<u64> {
        returns::returns_value(ctx)
    }

    pub fn returns_optional(ctx: Context<Returning>) -> Result<Option<u64>> {
        returns::returns_optional(ctx)
    }

    pub fn emits(ctx: Context<Returning>) -> Result {
        returns::emits(ctx)
    }

    pub fn cpi_emit(ctx: Context<CpiEmit>) -> Result {
        cpi_events::cpi_emit(ctx)
    }
}
