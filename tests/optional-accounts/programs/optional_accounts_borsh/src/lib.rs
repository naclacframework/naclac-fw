use naclac_lang::prelude::*;

declare_id!("13AwmpZtsjATXdwGx7wLkH9bU5sw9sL9KBqizRTau1Yd");

pub mod components;
pub mod constants;
pub mod instructions;

use instructions::*;

#[program]
pub mod optional_accounts_borsh {
    pub fn init_thing_a(ctx: Context<InitThingA>) -> Result {
        init_thing_a::init_thing_a(ctx)
    }

    pub fn init_thing_b(ctx: Context<InitThingB>) -> Result {
        init_thing_b::init_thing_b(ctx)
    }

    pub fn touch_optional(ctx: Context<TouchOptional>) -> Result {
        touch_optional::touch_optional(ctx)
    }

    pub fn touch_two_optional(ctx: Context<TouchTwoOptional>) -> Result {
        touch_two_optional::touch_two_optional(ctx)
    }

    pub fn init_optional_thing(ctx: Context<InitOptionalThing>) -> Result {
        init_optional_thing::init_optional_thing(ctx)
    }

    pub fn close_optional_thing(ctx: Context<CloseOptionalThing>) -> Result {
        close_optional_thing::close_optional_thing(ctx)
    }

    pub fn realloc_optional_thing(ctx: Context<ReallocOptionalThing>, new_space: u64) -> Result {
        realloc_optional_thing::realloc_optional_thing(ctx, new_space)
    }

    pub fn touch_boxed_optional(ctx: Context<TouchBoxedOptional>) -> Result {
        touch_boxed_optional::touch_boxed_optional(ctx)
    }
}
