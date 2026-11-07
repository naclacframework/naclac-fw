//! Phase 4: tracks fixed-point decimal scales through arithmetic and reports mismatches.

use {
    std::collections::HashMap,
    syn::{BinOp, Error, Expr, ExprLit, Lit, Pat, PatIdent, PatType},
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Scale {
    Known(u8),
    Unknown,
}

pub(super) struct ScaleTracker {
    /// Current known scale of each in-scope variable.
    var_scales: HashMap<String, Scale>,
    pub(super) errors: Vec<Error>,
}

impl ScaleTracker {
    pub(super) fn new(initial: HashMap<String, u8>) -> Self {
        Self {
            var_scales: initial
                .into_iter()
                .map(|(k, v)| (k, Scale::Known(v)))
                .collect(),
            errors: Vec::new(),
        }
    }

    pub(super) fn analyze_block(&mut self, block: &syn::Block) {
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
