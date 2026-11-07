//! The signer, writable, executable and rent-exempt checks of a field.

use {crate::accounts::field::parser::ParsedField, proc_macro2::TokenStream, quote::quote};

/// The checks for the flags written on `field`. On pinocchio the signer,
/// writable and executable checks are enforced by `load_and_validate`'s
/// declared-account walk before any loader runs, so only the other backends
/// emit them here.
pub(super) fn header_checks(field: &ParsedField) -> Vec<TokenStream> {
    let idx = field.index;
    let mut checks = Vec::new();

    if field.is_signer {
        let ty = &field.ty;
        checks.push(quote! {
            #[cfg(feature = "solana")]
            {
                if !<#ty as naclac_lang::prelude::NaclacAccount>::IS_SIGNER && !info.is_signer {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintSigner.err(#idx));
                }
            }
        });
    }

    if field.is_mut {
        checks.push(quote! {
            #[cfg(feature = "solana")]
            {
                if !info.is_writable {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintMut.err(#idx));
                }
            }
        });
    }

    if field.is_executable {
        checks.push(quote! {
            #[cfg(feature = "solana")]
            {
                if !info.executable {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintExecutable.err(#idx));
                }
            }
        });
    }

    if field.is_rent_exempt {
        checks.push(quote! {
            let __rent_exempt_lamports = info.lamports();
            #[cfg(feature = "solana")]
            let __rent_exempt_data_len = info.try_borrow_data()?.len();
            #[cfg(not(feature = "solana"))]
            let __rent_exempt_data_len = info.data().len();

            let __is_rent_exempt = __rent_exempt_lamports
                >= __naclac_rent.minimum_balance(__rent_exempt_data_len)?;

            if !__is_rent_exempt {
                return Err(naclac_lang::prelude::NaclacError::ConstraintRentExempt.err(#idx));
            }
        });
    }

    checks
}
