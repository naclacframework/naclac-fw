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
//! ```text
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

mod args;
mod cu_estimate;
mod float_ban;
mod fuzz;
mod kani;
mod literal_check;
mod math_rewrite;
mod scale;

use {
    args::parse_args,
    cu_estimate::CuEstimator,
    float_ban::FloatBanVisitor,
    fuzz::generate_fuzz_tests,
    kani::generate_kani_harness,
    literal_check::LiteralCheckVisitor,
    math_rewrite::MathRewriter,
    proc_macro::TokenStream,
    quote::quote,
    scale::ScaleTracker,
    syn::{
        parse_macro_input, visit::Visit, visit_mut::VisitMut, Error, FnArg, ItemFn, Pat, PatIdent,
        PatType, ReturnType, Type,
    },
};

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

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input_fn = parse_macro_input!(item as ItemFn);

    let args = match parse_args(attr) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };

    let fn_name_str = input_fn.sig.ident.to_string();

    // Phase 1: read-only static analysis.

    let mut phase1_errors: Vec<Error> = Vec::new();

    {
        let mut v = FloatBanVisitor {
            errors: Vec::new(),
            aliases: crate::common::crate_source::crate_aliases(),
        };
        v.visit_item_fn(&input_fn);
        phase1_errors.extend(v.errors);
    }

    {
        let mut v = LiteralCheckVisitor { errors: Vec::new() };
        v.visit_item_fn(&input_fn);
        phase1_errors.extend(v.errors);
    }

    if let Some(combined) = combine_errors(phase1_errors) {
        return combined.to_compile_error().into();
    }

    // Estimated before rewriting, so the count reflects the operations as written.
    let mut estimator = CuEstimator::default();
    estimator.visit_item_fn(&input_fn);

    // Phase 4: decimal scale tracking, before rewriting so the original expressions are seen.

    if !args.scales.is_empty() {
        let mut tracker = ScaleTracker::new(args.scales.clone());
        tracker.analyze_block(&input_fn.block);
        if let Some(combined) = combine_errors(tracker.errors) {
            return combined.to_compile_error().into();
        }
    }

    // Phase 2: math rewriting.

    if !args.no_rewrite {
        // The rewriter injects `?` — the function must return Result.
        if !returns_result(&input_fn) {
            return Error::new_spanned(
                &input_fn.sig.output,
                "Naclac Error: a #[system] function must return `Result<T>`, as safe math \
                 injects `?`. Change the return type, or use `#[system(no_rewrite)]`.",
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

    // Phase 3: companion fuzz and proof code.

    let fuzz_tests = generate_fuzz_tests(&input_fn, &args);
    let kani_harness = generate_kani_harness(&input_fn, &args);

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

    let fn_vis = &input_fn.vis;
    let fn_sig = &input_fn.sig;
    let fn_block = &input_fn.block;
    let fn_attrs = &input_fn.attrs; // preserve other attributes (e.g. #[inline])

    quote! {
        #cu_doc
        #(#fn_attrs)*
        #fn_vis #fn_sig {
            #[cfg(all(feature = "debug-mode", feature = "solana"))]
            naclac_lang::solana_log::sol_log_compute_units();
            #[cfg(all(feature = "debug-mode", not(feature = "solana")))]
            naclac_lang::prelude::sol_log_compute_units();

            let __result = { #fn_block };

            #[cfg(all(feature = "debug-mode", feature = "solana"))]
            naclac_lang::solana_log::sol_log_compute_units();
            #[cfg(all(feature = "debug-mode", not(feature = "solana")))]
            naclac_lang::prelude::sol_log_compute_units();

            __result
        }

        #fuzz_tests
        #kani_harness
    }
    .into()
}
