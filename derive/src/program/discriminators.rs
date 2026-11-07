//! The instruction discriminator as the dispatcher sees it: the expression
//! that reads it from the instruction data and the match-arm pattern that
//! matches it, both of the same integer type.

use quote::quote;

/// Tokens evaluating to the leading `width`-byte instruction discriminator as
/// an integer, for `instruction_data` already checked to hold at least `width`
/// bytes. Widths 1, 2, 4 and 8 read one native integer; any other width is
/// zero-padded into a `u64`. Must produce the same integer type and value
/// as `disc_arm_literal` builds for the same width.
pub(super) fn disc_read_tokens(width: usize) -> proc_macro2::TokenStream {
    match width {
        1 => quote! { instruction_data[0] },
        2 => quote! { u16::from_le_bytes([instruction_data[0], instruction_data[1]]) },
        4 => quote! {
            u32::from_le_bytes([
                instruction_data[0],
                instruction_data[1],
                instruction_data[2],
                instruction_data[3],
            ])
        },
        8 => quote! {
            u64::from_le_bytes(unsafe { *(instruction_data.as_ptr() as *const [u8; 8]) })
        },
        n => quote! {{
            let mut __disc_buf = [0u8; 8];
            __disc_buf[..#n].copy_from_slice(&instruction_data[..#n]);
            u64::from_le_bytes(__disc_buf)
        }},
    }
}

/// The match-arm pattern for a discriminator of `width` bytes, with the same
/// integer type and byte order as `disc_read_tokens(width)`.
pub(super) fn disc_arm_literal(width: usize, bytes: &[u8]) -> proc_macro2::TokenStream {
    match width {
        1 => {
            let value = bytes[0];
            quote! { #value }
        }
        2 => {
            let value = u16::from_le_bytes([bytes[0], bytes[1]]);
            quote! { #value }
        }
        4 => {
            let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            quote! { #value }
        }
        _ => {
            let mut padded = [0u8; 8];
            padded[..width].copy_from_slice(bytes);
            let value = u64::from_le_bytes(padded);
            quote! { #value }
        }
    }
}

/// The discriminator of every public handler, and their common width.
pub(super) struct ResolvedDiscriminators {
    pub(super) by_name: std::collections::HashMap<String, Vec<u8>>,
    pub(super) len: usize,
}

/// Resolves the discriminator of every public handler in `content_items`
/// (its `#[discriminator]` override, else the name-derived default) and checks
/// them, together with the crate's account and event discriminators, for
/// invalid or clashing values. `Err` holds the compile errors to emit.
pub(super) fn resolve(
    content_items: &[syn::Item],
) -> Result<ResolvedDiscriminators, proc_macro2::TokenStream> {
    use syn::Item;
    let mut ix_disc_by_name: std::collections::HashMap<String, Vec<u8>> =
        std::collections::HashMap::new();
    let mut ix_disc_entries: Vec<naclac_syn::discriminator::DiscriminatorEntry> = Vec::new();
    for item in content_items {
        let Item::Fn(func) = item else { continue };
        if !matches!(func.vis, syn::Visibility::Public(_)) {
            continue;
        }
        let name = func.sig.ident.to_string();
        let override_bytes = match naclac_syn::discriminator::parse_handler_override(&func.attrs) {
            Ok(bytes) => bytes,
            Err(err) => return Err(err.to_compile_error()),
        };
        let bytes = naclac_syn::discriminator::resolve_discriminator(
            naclac_syn::discriminator::DiscriminatorKind::Instruction,
            &name,
            override_bytes.as_deref(),
        );
        ix_disc_by_name.insert(name.clone(), bytes.clone());
        ix_disc_entries.push(naclac_syn::discriminator::DiscriminatorEntry { name, bytes });
    }
    let ix_disc_len = ix_disc_entries.first().map_or(
        naclac_syn::discriminator::DEFAULT_DISCRIMINATOR_LEN,
        |entry| entry.bytes.len(),
    );

    let mut discriminator_errors: Vec<String> = Vec::new();
    if let Err(errs) = naclac_syn::discriminator::validate_category(
        naclac_syn::discriminator::DiscriminatorKind::Instruction,
        &ix_disc_entries,
    ) {
        discriminator_errors.extend(errs);
    }
    {
        let found = naclac_syn::discriminator::scan_crate_discriminators(
            &crate::common::crate_source::manifest_dir(),
        );
        for (kind, entries) in [
            (
                naclac_syn::discriminator::DiscriminatorKind::Account,
                &found.accounts,
            ),
            (
                naclac_syn::discriminator::DiscriminatorKind::Event,
                &found.events,
            ),
        ] {
            if let Err(errs) = naclac_syn::discriminator::validate_category(kind, entries) {
                discriminator_errors.extend(errs);
            }
        }
    }
    if !discriminator_errors.is_empty() {
        let errors = discriminator_errors
            .iter()
            .map(|msg| syn::Error::new(proc_macro2::Span::call_site(), msg).to_compile_error());
        return Err(quote! { #(#errors)* });
    }

    Ok(ResolvedDiscriminators {
        by_name: ix_disc_by_name,
        len: ix_disc_len,
    })
}
