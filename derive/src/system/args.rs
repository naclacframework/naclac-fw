//! The arguments of `#[system(...)]`: parsing them into `SystemArgs`.

use {
    proc_macro::TokenStream,
    std::collections::HashMap,
    syn::{
        parse::{Parse, ParseStream},
        Error, LitInt, Token,
    },
};

#[derive(Default, Debug)]
pub(super) struct SystemArgs {
    /// Skip the math-rewriting pass (float ban and literal checks still run).
    pub(super) no_rewrite: bool,
    /// Rounding direction applied to every division in the function.
    pub(super) rounding: Option<RoundingMode>,
    /// Boolean invariant expression string for proptest fuzz generation.
    pub(super) invariant: Option<String>,
    /// Generate a Kani formal-verification proof harness.
    pub(super) generate_kani: bool,
    /// Fixed-point decimal scales keyed by parameter name.
    pub(super) scales: HashMap<String, u8>,
    /// Overrides the error the rewritten checked-math ops raise on failure
    /// (default: `naclac_lang::prelude::NaclacError::ArithmeticOverflow`).
    /// Set via `#[system(error = "path::to::MyError::Variant")]` when a
    /// program needs a specific error code out of its own error enum
    /// instead of the framework's generic one.
    pub(super) error_override: Option<syn::Path>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum RoundingMode {
    Up,
    Down,
}

struct ArgsParser(SystemArgs);

impl Parse for ArgsParser {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = SystemArgs::default();

        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;

            match ident.to_string().as_str() {
                "no_rewrite" => {
                    args.no_rewrite = true;
                }

                "kani" => {
                    args.generate_kani = true;
                }

                "rounding" => {
                    input.parse::<Token![=]>()?;
                    let lit: syn::LitStr = input.parse()?;
                    args.rounding = match lit.value().as_str() {
                        "up" => Some(RoundingMode::Up),
                        "down" => Some(RoundingMode::Down),
                        other => {
                            return Err(Error::new(
                                lit.span(),
                                format!(
                                    "Naclac Error: unknown rounding mode '{other}'; expected \
                                     \"up\" or \"down\"."
                                ),
                            ))
                        }
                    };
                }

                "invariant" => {
                    input.parse::<Token![=]>()?;
                    let lit: syn::LitStr = input.parse()?;
                    args.invariant = Some(lit.value());
                }

                "error" => {
                    input.parse::<Token![=]>()?;
                    let lit: syn::LitStr = input.parse()?;
                    let path: syn::Path = syn::parse_str(&lit.value()).map_err(|_| {
                        Error::new(
                            lit.span(),
                            format!(
                                "Naclac Error: can't parse `error = \"{}\"` as a Rust path; \
                                 expected e.g. \"crate::errors::MyError::Variant\".",
                                lit.value()
                            ),
                        )
                    })?;
                    args.error_override = Some(path);
                }

                "scale" => {
                    let content;
                    syn::parenthesized!(content in input);
                    while !content.is_empty() {
                        let param: syn::Ident = content.parse()?;
                        content.parse::<Token![=]>()?;
                        let scale_lit: LitInt = content.parse()?;
                        let scale_val: u8 = scale_lit.base10_parse().map_err(|_| {
                            Error::new(
                                scale_lit.span(),
                                "Naclac Error: scale value must be an integer in 0..=255.",
                            )
                        })?;
                        args.scales.insert(param.to_string(), scale_val);
                        if !content.is_empty() {
                            content.parse::<Token![,]>()?;
                        }
                    }
                }

                other => {
                    return Err(Error::new(
                        ident.span(),
                        format!(
                            "Naclac Error: unknown #[system] option '{other}'; valid options: \
                             no_rewrite, kani, rounding, invariant, scale(...), error."
                        ),
                    ));
                }
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(ArgsParser(args))
    }
}

pub(super) fn parse_args(attr: TokenStream) -> Result<SystemArgs, Error> {
    if attr.is_empty() {
        return Ok(SystemArgs::default());
    }
    let parsed: ArgsParser = syn::parse(attr)?;
    Ok(parsed.0)
}
