use naclac_lang::prelude::*;

/// `naclac_syn::discriminator::event_dispatch_tag(8)`.
const EVENT_DISPATCH_TAG: [u8; 8] = [34, 131, 189, 8, 189, 141, 224, 48];

/// Exercises `#[event_cpi]`'s auto-injected `event_authority`/`program`
/// fields and the reserved-tag receive arm end to end: the handler signs a
/// self-CPI carrying `tag ++ payload` with the injected `event_authority`.
#[event_cpi]
#[derive(Accounts)]
pub struct EventCpiSmoke {}

pub fn event_cpi_smoke(ctx: Context<EventCpiSmoke>) -> Result {
    let mut data = [0u8; 12];
    data[..8].copy_from_slice(&EVENT_DISPATCH_TAG);
    data[8..].copy_from_slice(&[1, 2, 3, 4]);

    let ix = solana_instruction::Instruction {
        program_id: *ctx.program_id,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(
            ctx.accounts.event_authority.address(),
            true,
        )],
        data: data.to_vec(),
    };
    cpi::invoke_signed(
        &ix,
        &[ctx.accounts.event_authority.to_cpi_handle()],
        &[&[b"__event_authority", &[ctx.bumps.event_authority]]],
    )
}
