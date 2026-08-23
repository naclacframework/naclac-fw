//! # System Macro Logic
//!
//! Handles the expansion of the `#[system]` macro. This is a comprehensive
//! compile-time math safety and analysis engine for Naclac smart contracts.
//!
//! ## Features
//!
//! 1.  **Float Ban** — Rejects `f32`/`f64` types and float literals.
//!     Floating-point arithmetic is non-deterministic on Solana/SBF validators.
//!
//! 2.  **Literal Overflow Detection** — Evaluates constant expressions at compile
//!     time and fails the build if the result overflows or divides by zero.
//!
//! 3.  **Safe Math Rewriting** — Transforms `+`, `-`, `*`, `/`, `%` into their
//!     `checked_*` counterparts. Raises `NaclacError::ArithmeticOverflow` by
//!     default, `#[system(error = "path::to::Error")]`'s error for the whole
//!     function, or `with_error!("path::to::Error", expr)`'s error for just
//!     that one expression (most specific wins; the marker itself is fully
//!     consumed by the rewrite, it's not a real macro).
//!
//! 4.  **Compound Assignment Rewriting** — Same as above for `+=`, `-=`, `*=`, `/=`, `%=`.
//!
//! 5.  **Bitwise Shift Optimization** — Replaces `* N` and `/ N` (where N is a power
//!     of two) with `<< k` and `>> k` to reduce Compute Unit consumption.
//!
//! 6.  **Rounding Control** — Via `#[system(rounding = "up")]`, enforces ceiling
//!     division for all `/` and `/=` operations in the function.
//!
//! 7.  **Static CU Estimation** — Counts arithmetic operations and annotates the
//!     function with a doc comment reporting the estimated worst-case Compute Unit cost.
//!
//! 8.  **Fuzz Test Generation** — Via `#[system(invariant = "expr")]`, generates a
//!     `proptest`-based fuzz test module that verifies the invariant for all inputs.
//!
//! 9.  **Kani Formal Verification** — Via `#[system(kani)]`, generates a
//!     `#[cfg(kani)]` proof harness so `cargo kani` can formally verify the function.
//!
//! 10. **Decimal Scale Tracking** — Via `#[system(scale(param = N))]`, tracks
//!     fixed-point decimal scales through arithmetic and emits errors on mismatches.
//!
//! ## Attribute Syntax
//!
//! ```rust
//! #[system]                                         // basic: float ban + safe math
//! #[system(no_rewrite)]                             // disable math rewriting only
//! #[system(rounding = "up")]                        // ceiling division everywhere
//! #[system(rounding = "down")]                      // floor division (default)
//! #[system(invariant = "result <= pool_y")]         // generate proptest fuzz tests
//! #[system(kani)]                                   // generate Kani proof harness
//! #[system(scale(balance = 6, rate = 9))]           // track decimal scales
//! // Attributes can be combined:
//! #[system(rounding = "up", invariant = "result <= y", kani)]
//! ```

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    spanned::Spanned,
    visit::Visit,
    visit_mut::VisitMut,
    BinOp, Error, Expr, ExprBinary, ExprLit, FnArg, ItemFn, Lit, LitInt, Pat, PatIdent, PatType,
    ReturnType, Token, Type,
};

// ═══════════════════════════════════════════════════════════════════════════
//  § 1  Attribute Parsing
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Default, Debug)]
struct SystemArgs {
    /// Skip the math-rewriting pass (float ban and literal checks still run).
    no_rewrite: bool,
    /// Rounding direction applied to every division in the function.
    rounding: Option<RoundingMode>,
    /// Boolean invariant expression string for proptest fuzz generation.
    invariant: Option<String>,
    /// Generate a Kani formal-verification proof harness.
    generate_kani: bool,
    /// Fixed-point decimal scales keyed by parameter name.
    scales: HashMap<String, u8>,
    /// Overrides the error the rewritten checked-math ops raise on failure
    /// (default: `naclac_lang::prelude::NaclacError::ArithmeticOverflow`).
    /// Set via `#[system(error = "path::to::MyError::Variant")]` when a
    /// program needs a specific error code out of its own error enum
    /// instead of the framework's generic one.
    error_override: Option<syn::Path>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum RoundingMode {
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
                                    "[naclac::system] Unknown rounding mode '{other}'.\n\
                                     Expected \"up\" (ceiling division) or \"down\" (floor division)."
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
                                "[naclac::system] Could not parse `error = \"{}\"` as a Rust \
                                 path. Expected something like \"crate::errors::MyError::Variant\".",
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
                        let scale_val: u8 =
                            scale_lit.base10_parse().map_err(|_| {
                                Error::new(
                                    scale_lit.span(),
                                    "[naclac::system] scale value must be an integer in the range 0–255.",
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
                            "[naclac::system] Unknown attribute '{other}'.\n\
                             Valid options: no_rewrite, kani, rounding, invariant, scale(...), error."
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

fn parse_args(attr: TokenStream) -> Result<SystemArgs, Error> {
    if attr.is_empty() {
        return Ok(SystemArgs::default());
    }
    let parsed: ArgsParser = syn::parse(attr)?;
    Ok(parsed.0)
}

// ═══════════════════════════════════════════════════════════════════════════
//  § 2  Phase 1a — Float-Ban Visitor
// ═══════════════════════════════════════════════════════════════════════════

struct FloatBanVisitor {
    errors: Vec<Error>,
}

impl<'ast> Visit<'ast> for FloatBanVisitor {
    fn visit_type(&mut self, ty: &'ast Type) {
        if let Type::Path(type_path) = ty {
            if let Some(seg) = type_path.path.segments.last() {
                let name = seg.ident.to_string();
                if name == "f32" || name == "f64" {
                    self.errors.push(Error::new_spanned(
                        ty,
                        format!(
                            "[naclac::system] Floating-point type `{name}` is not allowed.\n\
                             Floating-point arithmetic is non-deterministic across Solana/SBF \
                             validators and will produce inconsistent state.\n\
                             Use integer arithmetic with a fixed decimal scale instead."
                        ),
                    ));
                }
            }
        }
        syn::visit::visit_type(self, ty);
    }

    fn visit_lit(&mut self, lit: &'ast Lit) {
        if let Lit::Float(f) = lit {
            self.errors.push(Error::new_spanned(
                f,
                "[naclac::system] Floating-point literal is not allowed.\n\
                 Floating-point arithmetic is non-deterministic across Solana/SBF validators.\n\
                 Use an integer literal with a fixed decimal scale instead\n\
                 (e.g. `0.5` → `5_000u64` with scale 4, or use basis points).",
            ));
        }
        syn::visit::visit_lit(self, lit);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  § 3  Phase 1b — Literal Overflow & Division-by-Zero Visitor
// ═══════════════════════════════════════════════════════════════════════════

struct LiteralCheckVisitor {
    errors: Vec<Error>,
}

impl<'ast> Visit<'ast> for LiteralCheckVisitor {
    fn visit_expr_binary(&mut self, expr: &'ast ExprBinary) {
        // Division by literal zero
        let is_div_or_rem = matches!(
            &expr.op,
            BinOp::Div(_) | BinOp::Rem(_) | BinOp::DivAssign(_) | BinOp::RemAssign(_)
        );
        if is_div_or_rem {
            if let Expr::Lit(ExprLit {
                lit: Lit::Int(i), ..
            }) = expr.right.as_ref()
            {
                if i.base10_digits() == "0" {
                    self.errors.push(Error::new_spanned(
                        expr,
                        "[naclac::system] Division or remainder by literal zero detected.\n\
                         This would cause a panic at runtime. Remove or guard this operation.",
                    ));
                }
            }
        }

        // Constant integer overflow: both operands are integer literals
        if let (
            Expr::Lit(ExprLit {
                lit: Lit::Int(lhs), ..
            }),
            Expr::Lit(ExprLit {
                lit: Lit::Int(rhs), ..
            }),
        ) = (expr.left.as_ref(), expr.right.as_ref())
        {
            if let (Ok(l), Ok(r)) = (lhs.base10_parse::<u128>(), rhs.base10_parse::<u128>()) {
                let result: Option<Option<u128>> = match &expr.op {
                    BinOp::Add(_) | BinOp::AddAssign(_) => Some(l.checked_add(r)),
                    BinOp::Sub(_) | BinOp::SubAssign(_) => Some(l.checked_sub(r)),
                    BinOp::Mul(_) | BinOp::MulAssign(_) => Some(l.checked_mul(r)),
                    BinOp::Div(_) | BinOp::DivAssign(_) => Some(l.checked_div(r)),
                    BinOp::Rem(_) | BinOp::RemAssign(_) => Some(l.checked_rem(r)),
                    _ => None,
                };

                if result == Some(None) {
                    let sym = op_symbol(&expr.op);
                    self.errors.push(Error::new_spanned(
                        expr,
                        format!(
                            "[naclac::system] Constant expression `{l} {sym} {r}` overflows \
                             or is undefined at compile time."
                        ),
                    ));
                }
            }
        }

        syn::visit::visit_expr_binary(self, expr);
    }
}

fn op_symbol(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add(_) | BinOp::AddAssign(_) => "+",
        BinOp::Sub(_) | BinOp::SubAssign(_) => "-",
        BinOp::Mul(_) | BinOp::MulAssign(_) => "*",
        BinOp::Div(_) | BinOp::DivAssign(_) => "/",
        BinOp::Rem(_) | BinOp::RemAssign(_) => "%",
        _ => "op",
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  § 4  Phase 1c — Static CU Estimator
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Default)]
struct CuEstimator {
    add: usize,
    sub: usize,
    mul: usize,
    div: usize,
    rem: usize,
}

impl CuEstimator {
    // Approximate SBF CU weights per operation type.
    fn total_cu(&self) -> usize {
        self.add + self.sub + self.mul * 5 + self.div * 10 + self.rem * 10
    }

    fn summary(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.add > 0 {
            parts.push(format!("{}× add", self.add));
        }
        if self.sub > 0 {
            parts.push(format!("{}× sub", self.sub));
        }
        if self.mul > 0 {
            parts.push(format!("{}× mul", self.mul));
        }
        if self.div > 0 {
            parts.push(format!("{}× div", self.div));
        }
        if self.rem > 0 {
            parts.push(format!("{}× rem", self.rem));
        }
        if parts.is_empty() {
            return "no arithmetic".to_string();
        }
        parts.join(", ")
    }
}

impl<'ast> Visit<'ast> for CuEstimator {
    fn visit_expr_binary(&mut self, expr: &'ast ExprBinary) {
        match &expr.op {
            BinOp::Add(_) | BinOp::AddAssign(_) => self.add += 1,
            BinOp::Sub(_) | BinOp::SubAssign(_) => self.sub += 1,
            BinOp::Mul(_) | BinOp::MulAssign(_) => self.mul += 1,
            BinOp::Div(_) | BinOp::DivAssign(_) => self.div += 1,
            BinOp::Rem(_) | BinOp::RemAssign(_) => self.rem += 1,
            _ => {}
        }
        syn::visit::visit_expr_binary(self, expr);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  § 5  Phase 2 — Math Rewriter (VisitMut)
// ═══════════════════════════════════════════════════════════════════════════

struct MathRewriter {
    rounding: Option<RoundingMode>,
    /// The error expression every rewritten `.ok_or(...)` raises on failure —
    /// `naclac_lang::prelude::NaclacError::ArithmeticOverflow` by default, or
    /// whatever `#[system(error = "...")]` supplied. Temporarily swapped out
    /// while visiting inside a `with_error!("...", expr)` marker.
    error_expr: TokenStream2,
    errors: Vec<Error>,
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
                    Ok(WithErrorArgs { error_lit, mut inner }) => {
                        match syn::parse_str::<syn::Path>(&error_lit.value()) {
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
                                        "[naclac::system] Could not parse `with_error!(\"{}\", ...)`'s \
                                         first argument as a Rust path.",
                                        error_lit.value()
                                    ),
                                ));
                            }
                        }
                    }
                    Err(e) => self.errors.push(Error::new(
                        e.span(),
                        "[naclac::system] `with_error!` expects exactly `with_error!(\"path::to::Error\", expr)`.",
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

// ═══════════════════════════════════════════════════════════════════════════
//  § 6  Phase 4 — Decimal Scale Tracker
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Scale {
    Known(u8),
    Unknown,
}

struct ScaleTracker {
    /// Current known scale of each in-scope variable.
    var_scales: HashMap<String, Scale>,
    errors: Vec<Error>,
}

impl ScaleTracker {
    fn new(initial: HashMap<String, u8>) -> Self {
        Self {
            var_scales: initial
                .into_iter()
                .map(|(k, v)| (k, Scale::Known(v)))
                .collect(),
            errors: Vec::new(),
        }
    }

    fn analyze_block(&mut self, block: &syn::Block) {
        for stmt in &block.stmts {
            self.analyze_stmt(stmt);
        }
    }

    fn analyze_stmt(&mut self, stmt: &syn::Stmt) {
        match stmt {
            // let x = expr;  or  let x: T = expr;
            syn::Stmt::Local(local) => {
                let scale = local
                    .init
                    .as_ref()
                    .map(|i| self.eval_scale(&i.expr))
                    .unwrap_or(Scale::Unknown);
                self.bind_pat(&local.pat, scale);
            }
            // standalone expression / assignment
            syn::Stmt::Expr(expr, _) => {
                self.analyze_assign(expr);
            }
            _ => {}
        }
    }

    fn bind_pat(&mut self, pat: &Pat, scale: Scale) {
        match pat {
            Pat::Ident(PatIdent { ident, .. }) => {
                self.var_scales.insert(ident.to_string(), scale);
            }
            Pat::Type(PatType { pat, .. }) => self.bind_pat(pat, scale),
            _ => {}
        }
    }

    fn analyze_assign(&mut self, expr: &Expr) {
        if let Expr::Assign(a) = expr {
            let scale = self.eval_scale(&a.right);
            if let Expr::Path(p) = a.left.as_ref() {
                if let Some(ident) = p.path.get_ident() {
                    self.var_scales.insert(ident.to_string(), scale);
                }
            }
        }
    }

    fn eval_scale(&mut self, expr: &Expr) -> Scale {
        match expr {
            // A named variable: look up its tracked scale.
            Expr::Path(p) => p
                .path
                .get_ident()
                .and_then(|id| self.var_scales.get(&id.to_string()).copied())
                .unwrap_or(Scale::Unknown),

            // Integer literals are dimensionless (scale 0).
            Expr::Lit(ExprLit {
                lit: Lit::Int(_), ..
            }) => Scale::Known(0),

            // Transparent wrappers — propagate through.
            Expr::Paren(e) => self.eval_scale(&e.expr),
            Expr::Cast(e) => self.eval_scale(&e.expr),
            Expr::Reference(e) => self.eval_scale(&e.expr),
            Expr::Unary(e) => self.eval_scale(&e.expr),
            Expr::MethodCall(mc) => self.eval_scale(&mc.receiver),

            // Arithmetic — apply scale-propagation rules.
            Expr::Binary(bin) => {
                // Clone to avoid re-borrowing issues while calling eval_scale.
                let left_expr = (*bin.left).clone();
                let right_expr = (*bin.right).clone();
                let ls = self.eval_scale(&left_expr);
                let rs = self.eval_scale(&right_expr);

                match &bin.op {
                    // Addition / subtraction: scales must match.
                    BinOp::Add(_) | BinOp::Sub(_) | BinOp::AddAssign(_) | BinOp::SubAssign(_) => {
                        match (ls, rs) {
                            (Scale::Known(l), Scale::Known(r)) if l == r => Scale::Known(l),
                            (Scale::Known(l), Scale::Known(r)) => {
                                self.errors.push(Error::new_spanned(
                                    bin,
                                    format!(
                                        "[naclac::system] Decimal scale mismatch.\n\
                                     Left operand has {l} decimal places, \
                                     right operand has {r} decimal places.\n\
                                     You cannot add or subtract values with different scales.\n\
                                     Normalize both values to the same scale first."
                                    ),
                                ));
                                Scale::Unknown
                            }
                            (Scale::Known(s), _) | (_, Scale::Known(s)) => Scale::Known(s),
                            _ => Scale::Unknown,
                        }
                    }

                    // Multiplication: scales add.
                    BinOp::Mul(_) | BinOp::MulAssign(_) => match (ls, rs) {
                        (Scale::Known(l), Scale::Known(r)) => Scale::Known(l.saturating_add(r)),
                        (Scale::Known(s), _) | (_, Scale::Known(s)) => Scale::Known(s),
                        _ => Scale::Unknown,
                    },

                    // Division: scales subtract (saturating to 0).
                    BinOp::Div(_) | BinOp::DivAssign(_) => match (ls, rs) {
                        (Scale::Known(l), Scale::Known(r)) => Scale::Known(l.saturating_sub(r)),
                        (Scale::Known(l), _) => Scale::Known(l),
                        _ => Scale::Unknown,
                    },

                    _ => Scale::Unknown,
                }
            }

            _ => Scale::Unknown,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  § 7  Phase 3a — Fuzz Test Generator
// ═══════════════════════════════════════════════════════════════════════════

fn generate_fuzz_tests(func: &ItemFn, args: &SystemArgs) -> TokenStream2 {
    let invariant_str = match &args.invariant {
        Some(s) => s.as_str(),
        None => return quote! {},
    };

    // Parse the invariant as a boolean Rust expression.
    let invariant_expr: Expr = match syn::parse_str(invariant_str) {
        Ok(e) => e,
        Err(_) => {
            let msg = format!(
                "[naclac::system] Could not parse invariant expression `{invariant_str}`.\n\
                 It must be a valid Rust boolean expression that may reference `result` \
                 and any of the function's parameter names."
            );
            return quote! { compile_error!(#msg); };
        }
    };

    let fn_name = &func.sig.ident;
    let mod_name = format_ident!("__naclac_fuzz_{}", fn_name);
    let invariant_lit = invariant_str;

    // Build `name: Type` parameter list for proptest.
    let param_decls: Vec<TokenStream2> = func
        .sig
        .inputs
        .iter()
        .filter_map(typed_param)
        .map(|(id, ty)| quote! { #id: #ty })
        .collect();

    let param_names: Vec<_> = func
        .sig
        .inputs
        .iter()
        .filter_map(typed_param)
        .map(|(id, _)| id)
        .collect();

    if param_decls.is_empty() {
        return quote! {};
    }

    quote! {
        #[cfg(test)]
        mod #mod_name {
            use super::*;

            proptest::proptest! {
                #[test]
                fn invariant_holds(#(#param_decls),*) {
                    if let Ok(result) = #fn_name(#(#param_names),*) {
                        proptest::prop_assert!(
                            { #invariant_expr },
                            "[naclac::system] Invariant `{}` violated. result = {:?}",
                            #invariant_lit,
                            result
                        );
                    }
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  § 8  Phase 3b — Kani Harness Generator
// ═══════════════════════════════════════════════════════════════════════════

fn generate_kani_harness(func: &ItemFn, args: &SystemArgs) -> TokenStream2 {
    if !args.generate_kani {
        return quote! {};
    }

    let fn_name = &func.sig.ident;
    let mod_name = format_ident!("__naclac_kani_{}", fn_name);
    let proof_fn = format_ident!("prove_{}_never_panics", fn_name);

    let bindings: Vec<TokenStream2> = func
        .sig
        .inputs
        .iter()
        .filter_map(typed_param)
        .map(|(id, ty)| {
            quote! { let #id: #ty = kani::any(); }
        })
        .collect();

    let param_names: Vec<_> = func
        .sig
        .inputs
        .iter()
        .filter_map(typed_param)
        .map(|(id, _)| id)
        .collect();

    quote! {
        #[cfg(kani)]
        mod #mod_name {
            use super::*;

            /// Automatically generated by `#[system(kani)]`.
            ///
            /// Run with: `cargo kani`
            ///
            /// Kani will exhaustively verify that this function never panics,
            /// overflows, or causes undefined behaviour for any valid input.
            #[kani::proof]
            fn #proof_fn() {
                #(#bindings)*
                let _ = #fn_name(#(#param_names),*);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  § 9  Helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Extract (ident, type) from a typed function argument, skipping receivers.
fn typed_param(arg: &FnArg) -> Option<(syn::Ident, Box<Type>)> {
    if let FnArg::Typed(PatType { pat, ty, .. }) = arg {
        if let Pat::Ident(PatIdent { ident, .. }) = pat.as_ref() {
            return Some((ident.clone(), ty.clone()));
        }
    }
    None
}

/// Returns `true` when the function's declared return type is `Result` (any
/// path whose last segment is exactly `Result`, e.g. the crate's own
/// `naclac_lang::prelude::Result` alias or a bare `Result` brought into scope
/// via that prelude glob).
fn returns_result(func: &ItemFn) -> bool {
    match &func.sig.output {
        ReturnType::Type(_, ty) => crate::type_classify::is_exactly(ty, "Result"),
        ReturnType::Default => false,
    }
}

/// Fold a `Vec<syn::Error>` into a single combined error, or return `None`.
fn combine_errors(errors: Vec<Error>) -> Option<Error> {
    let mut iter = errors.into_iter();
    let first = iter.next()?;
    Some(iter.fold(first, |mut acc, e| {
        acc.combine(e);
        acc
    }))
}

// ═══════════════════════════════════════════════════════════════════════════
//  § 10  Main Entry Point
// ═══════════════════════════════════════════════════════════════════════════

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    // ── Parse the function and its attribute arguments. ───────────────────
    let mut input_fn = parse_macro_input!(item as ItemFn);

    let args = match parse_args(attr) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };

    let fn_name_str = input_fn.sig.ident.to_string();

    // ═════════════════════════════════════════════════════════════════════
    //  PHASE 1: Static Analysis (read-only visitors)
    // ═════════════════════════════════════════════════════════════════════

    let mut phase1_errors: Vec<Error> = Vec::new();

    // 1a. Float ban
    {
        let mut v = FloatBanVisitor { errors: Vec::new() };
        v.visit_item_fn(&input_fn);
        phase1_errors.extend(v.errors);
    }

    // 1b. Literal overflow & div-by-zero
    {
        let mut v = LiteralCheckVisitor { errors: Vec::new() };
        v.visit_item_fn(&input_fn);
        phase1_errors.extend(v.errors);
    }

    // Emit all Phase-1 errors as a combined compile_error! and stop.
    if let Some(combined) = combine_errors(phase1_errors) {
        return combined.to_compile_error().into();
    }

    // 1c. CU estimation — run BEFORE rewriting to reflect the original op count.
    let mut estimator = CuEstimator::default();
    estimator.visit_item_fn(&input_fn);

    // ═════════════════════════════════════════════════════════════════════
    //  PHASE 4: Decimal Scale Tracking (before rewriting for accurate AST)
    // ═════════════════════════════════════════════════════════════════════

    if !args.scales.is_empty() {
        let mut tracker = ScaleTracker::new(args.scales.clone());
        tracker.analyze_block(&input_fn.block);
        if let Some(combined) = combine_errors(tracker.errors) {
            return combined.to_compile_error().into();
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    //  PHASE 2: Math Rewriting (mutating visitor)
    // ═════════════════════════════════════════════════════════════════════

    if !args.no_rewrite {
        // The rewriter injects `?` — the function must return Result.
        if !returns_result(&input_fn) {
            return Error::new_spanned(
                &input_fn.sig.output,
                "[naclac::system] Functions annotated with `#[system]` must return `Result<T>` \
                 because the safe-math rewriter injects the `?` operator for every arithmetic \
                 operation.\n\n\
                 Options:\n\
                 • Change the return type to `Result<T>` (recommended for on-chain math).\n\
                 • Use `#[system(no_rewrite)]` to opt out of the automatic rewrite.",
            )
            .to_compile_error()
            .into();
        }

        let error_expr = match &args.error_override {
            Some(path) => quote! { #path },
            None => quote! { naclac_lang::prelude::NaclacError::ArithmeticOverflow },
        };
        let mut rewriter = MathRewriter {
            rounding: args.rounding,
            error_expr,
            errors: Vec::new(),
        };
        rewriter.visit_item_fn_mut(&mut input_fn);
        if let Some(combined) = combine_errors(rewriter.errors) {
            return combined.to_compile_error().into();
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    //  PHASE 3: Companion Code Generation
    // ═════════════════════════════════════════════════════════════════════

    let fuzz_tests = generate_fuzz_tests(&input_fn, &args);
    let kani_harness = generate_kani_harness(&input_fn, &args);

    // ── CU Estimation: surface as a doc comment on the generated function ─
    let cu_doc = {
        let total = estimator.total_cu();
        if total > 0 {
            let msg = format!(
                " [naclac::system] '{fn_name_str}' — estimated arithmetic cost: \
                 ~{total} CU ({}). Weights: add/sub=1 CU, mul=5 CU, div/rem=10 CU.",
                estimator.summary()
            );
            quote! { #[doc = #msg] }
        } else {
            quote! {}
        }
    };

    // ── Reassemble the function with debug-mode CU profiling injected. ───
    let fn_vis = &input_fn.vis;
    let fn_sig = &input_fn.sig;
    let fn_block = &input_fn.block;
    let fn_attrs = &input_fn.attrs; // preserve other attributes (e.g. #[inline])

    quote! {
        #cu_doc
        #(#fn_attrs)*
        #fn_vis #fn_sig {
            // Before: log remaining CUs in debug mode.
            #[cfg(all(feature = "debug-mode", not(feature = "pinocchio")))]
            naclac_lang::solana_program::log::sol_log_compute_units();
            #[cfg(all(feature = "debug-mode", feature = "pinocchio"))]
            naclac_lang::prelude::sol_log_compute_units();

            let __result = { #fn_block };

            // After: log remaining CUs to measure cost of this system.
            #[cfg(all(feature = "debug-mode", not(feature = "pinocchio")))]
            naclac_lang::solana_program::log::sol_log_compute_units();
            #[cfg(all(feature = "debug-mode", feature = "pinocchio"))]
            naclac_lang::prelude::sol_log_compute_units();

            __result
        }

        #fuzz_tests
        #kani_harness
    }
    .into()
}
