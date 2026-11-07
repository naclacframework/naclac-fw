//! Phase 2: rewrites arithmetic into checked math, optimises power-of-two multiply/divide into shifts, and applies rounding.

use {
    super::args::RoundingMode,
    proc_macro2::TokenStream as TokenStream2,
    quote::quote,
    syn::{
        parse::{Parse, ParseStream},
        spanned::Spanned,
        visit_mut::VisitMut,
        BinOp, Error, Expr, ExprLit, Lit, LitInt, Token,
    },
};

pub(super) struct MathRewriter {
    pub(super) rounding: Option<RoundingMode>,
    /// The error expression every rewritten `.ok_or(...)` raises on failure —
    /// `naclac_lang::prelude::NaclacError::ArithmeticOverflow` by default, or
    /// whatever `#[system(error = "...")]` supplied. Temporarily swapped out
    /// while visiting inside a `with_error!("...", expr)` marker.
    pub(super) error_expr: TokenStream2,
    pub(super) errors: Vec<Error>,
}

/// `with_error!("path::to::Error", expr)` — scopes a one-off error override
/// to just `expr`, overriding the function-level `#[system(error = "...")]`
/// (or the framework default) for every checked op inside it. Consumed
/// entirely by the rewriter: `expr` is rewritten in place and the marker
/// itself never survives to real compilation.
struct WithErrorArgs {
    error_lit: syn::LitStr,
    inner: Expr,
}

impl Parse for WithErrorArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let error_lit: syn::LitStr = input.parse()?;
        input.parse::<Token![,]>()?;
        let inner: Expr = input.parse()?;
        Ok(WithErrorArgs { error_lit, inner })
    }
}

/// Strips one layer of source-level `(...)` grouping, if present. Grouping
/// parens that were only needed to fix precedence at their original call
/// site (e.g. `a / (b - c)`) become redundant once that operand is spliced
/// into its own `let` binding instead — keeping them there would trip
/// `unused_parens` on otherwise-correct generated code.
fn unwrap_paren(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(paren) => &paren.expr,
        _ => expr,
    }
}

/// If `expr` is an integer literal that is a power of two greater than 1,
/// returns the shift amount (trailing zero count). Otherwise returns None.
fn power_of_two_shift(expr: &Expr) -> Option<u32> {
    if let Expr::Lit(ExprLit {
        lit: Lit::Int(i), ..
    }) = expr
    {
        if let Ok(val) = i.base10_parse::<u64>() {
            if val > 1 && val.is_power_of_two() {
                return Some(val.trailing_zeros());
            }
        }
    }
    None
}

impl VisitMut for MathRewriter {
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        // `with_error!("...", inner)` — checked before the generic recursion
        // below so `inner` gets visited (and its own nested `with_error!`s,
        // if any, resolved) under the overridden error, then the marker is
        // replaced by its now-rewritten `inner` and the override reverted.
        if let Expr::Macro(expr_macro) = &expr {
            if expr_macro.mac.path.is_ident("with_error") {
                match syn::parse2::<WithErrorArgs>(expr_macro.mac.tokens.clone()) {
                    Ok(WithErrorArgs {
                        error_lit,
                        mut inner,
                    }) => match syn::parse_str::<syn::Path>(&error_lit.value()) {
                        Ok(path) => {
                            let saved = core::mem::replace(&mut self.error_expr, quote! { #path });
                            self.visit_expr_mut(&mut inner);
                            self.error_expr = saved;
                            *expr = inner;
                        }
                        Err(_) => {
                            self.errors.push(Error::new(
                                error_lit.span(),
                                format!(
                                    "Naclac Error: `with_error!(\"{}\", ...)` needs a Rust \
                                         path as its first argument.",
                                    error_lit.value()
                                ),
                            ));
                        }
                    },
                    Err(e) => self.errors.push(Error::new(
                        e.span(),
                        "Naclac Error: `with_error!` takes `with_error!(\"path::to::Error\", \
                         expr)`.",
                    )),
                }
                return;
            }
        }

        // Bottom-up: rewrite children before the parent so nested
        // expressions compose correctly, e.g. (a + b) * c.
        syn::visit_mut::visit_expr_mut(self, expr);

        let span = expr.span();
        let error_expr = &self.error_expr;

        // We need to inspect the expression and, if it is an arithmetic
        // binary expression, produce a replacement.  We collect the
        // replacement in an Option so we can release the borrow of `expr`
        // before calling `*expr = …`.
        let replacement: Option<Expr> = if let Expr::Binary(bin) = &*expr {
            let left = &*bin.left;
            let right = &*bin.right;

            match &bin.op {
                // ── Standard binary arithmetic ─────────────────────────
                BinOp::Add(_) => {
                    let unwrapped_right = unwrap_paren(right);
                    Some(syn::parse_quote_spanned! { span =>
                        (#left)
                            .checked_add(#unwrapped_right)
                            .ok_or(#error_expr)?
                    })
                }

                BinOp::Sub(_) => {
                    let unwrapped_right = unwrap_paren(right);
                    Some(syn::parse_quote_spanned! { span =>
                        (#left)
                            .checked_sub(#unwrapped_right)
                            .ok_or(#error_expr)?
                    })
                }

                BinOp::Mul(_) => {
                    if let Some(shift) = power_of_two_shift(right) {
                        // Bitshift optimisation: x * 8 → x << 3
                        let k = LitInt::new(&shift.to_string(), span);
                        Some(syn::parse_quote_spanned! { span => (#left << #k) })
                    } else {
                        let unwrapped_right = unwrap_paren(right);
                        Some(syn::parse_quote_spanned! { span =>
                            (#left)
                                .checked_mul(#unwrapped_right)
                                .ok_or(#error_expr)?
                        })
                    }
                }

                BinOp::Div(_) => {
                    if let Some(shift) = power_of_two_shift(right) {
                        // Bitshift optimisation: x / 8 → x >> 3
                        let k = LitInt::new(&shift.to_string(), span);
                        Some(syn::parse_quote_spanned! { span => (#left >> #k) })
                    } else if self.rounding == Some(RoundingMode::Up) {
                        // Ceiling division: ceil(a / b) = (a + b - 1) / b
                        // Entirely via checked ops so no intermediate overflow goes uncaught.
                        let unwrapped_left = unwrap_paren(left);
                        let unwrapped_right = unwrap_paren(right);
                        Some(syn::parse_quote_spanned! { span =>
                            {
                                let __b = #unwrapped_right;
                                let __a = #unwrapped_left;
                                __a.checked_add(__b)
                                    .ok_or(#error_expr)?
                                    .checked_sub(1)
                                    .ok_or(#error_expr)?
                                    .checked_div(__b)
                                    .ok_or(#error_expr)?
                            }
                        })
                    } else {
                        let unwrapped_right = unwrap_paren(right);
                        Some(syn::parse_quote_spanned! { span =>
                            (#left)
                                .checked_div(#unwrapped_right)
                                .ok_or(#error_expr)?
                        })
                    }
                }

                BinOp::Rem(_) => {
                    let unwrapped_right = unwrap_paren(right);
                    Some(syn::parse_quote_spanned! { span =>
                        (#left)
                            .checked_rem(#unwrapped_right)
                            .ok_or(#error_expr)?
                    })
                }

                // ── Compound assignments ───────────────────────────────
                // In syn 2.x, `a += b` is Expr::Binary { op: AddAssign }.
                // We rewrite `a op= b` → `a = (a).checked_op(b).ok_or(…)?`.
                // NOTE: If the left-hand side is a simple field/path (the
                // common case in smart-contract math), this is safe.  For
                // expressions with side-effects on the lhs, the developer
                // should use `#[system(no_rewrite)]` and write checked math
                // manually.
                BinOp::AddAssign(_) => {
                    let unwrapped_right = unwrap_paren(right);
                    Some(syn::parse_quote_spanned! { span =>
                        #left = (#left)
                            .checked_add(#unwrapped_right)
                            .ok_or(#error_expr)?
                    })
                }

                BinOp::SubAssign(_) => {
                    let unwrapped_right = unwrap_paren(right);
                    Some(syn::parse_quote_spanned! { span =>
                        #left = (#left)
                            .checked_sub(#unwrapped_right)
                            .ok_or(#error_expr)?
                    })
                }

                BinOp::MulAssign(_) => {
                    if let Some(shift) = power_of_two_shift(right) {
                        let k = LitInt::new(&shift.to_string(), span);
                        Some(syn::parse_quote_spanned! { span => #left <<= #k })
                    } else {
                        let unwrapped_right = unwrap_paren(right);
                        Some(syn::parse_quote_spanned! { span =>
                            #left = (#left)
                                .checked_mul(#unwrapped_right)
                                .ok_or(#error_expr)?
                        })
                    }
                }

                BinOp::DivAssign(_) => {
                    if let Some(shift) = power_of_two_shift(right) {
                        let k = LitInt::new(&shift.to_string(), span);
                        Some(syn::parse_quote_spanned! { span => #left >>= #k })
                    } else if self.rounding == Some(RoundingMode::Up) {
                        let unwrapped_right = unwrap_paren(right);
                        Some(syn::parse_quote_spanned! { span =>
                            {
                                let __b = #unwrapped_right;
                                #left = (#left)
                                    .checked_add(__b)
                                    .ok_or(#error_expr)?
                                    .checked_sub(1)
                                    .ok_or(#error_expr)?
                                    .checked_div(__b)
                                    .ok_or(#error_expr)?;
                            }
                        })
                    } else {
                        let unwrapped_right = unwrap_paren(right);
                        Some(syn::parse_quote_spanned! { span =>
                            #left = (#left)
                                .checked_div(#unwrapped_right)
                                .ok_or(#error_expr)?
                        })
                    }
                }

                BinOp::RemAssign(_) => {
                    let unwrapped_right = unwrap_paren(right);
                    Some(syn::parse_quote_spanned! { span =>
                        #left = (#left)
                            .checked_rem(#unwrapped_right)
                            .ok_or(#error_expr)?
                    })
                }

                _ => None,
            }
        } else {
            None
        };

        if let Some(new_expr) = replacement {
            *expr = new_expr;
        }
    }
}
