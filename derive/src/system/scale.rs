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

    /// Analyzes `block` with its own scope: a name it binds or reassigns does
    /// not change the scale of that name after the block.
    fn analyze_scoped_block(&mut self, block: &syn::Block) {
        let outer = self.var_scales.clone();
        self.analyze_block(block);
        self.var_scales = outer;
    }

    /// Evaluates `expr` with its own scope, for a branch or loop body.
    fn eval_scoped(&mut self, expr: &Expr) {
        let outer = self.var_scales.clone();
        self.eval_scale(expr);
        self.var_scales = outer;
    }

    fn analyze_stmt(&mut self, stmt: &syn::Stmt) {
        match stmt {
            syn::Stmt::Local(local) => {
                let scale = match &local.init {
                    Some(init) => {
                        let scale = self.eval_scale(&init.expr);
                        if let Some((_, diverge)) = &init.diverge {
                            self.eval_scoped(diverge);
                        }
                        scale
                    }
                    None => Scale::Unknown,
                };
                self.bind_pat(&local.pat, scale);
            }
            syn::Stmt::Expr(expr, _) => {
                self.eval_scale(expr);
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

    /// The scale of `expr`, reporting every scale mismatch anywhere inside it.
    /// Each sub-expression is evaluated exactly once, so a mismatch is
    /// reported once wherever it sits: a `let`, an assignment, a call or
    /// method argument, a returned value, a branch or a loop body.
    fn eval_scale(&mut self, expr: &Expr) -> Scale {
        match expr {
            Expr::Assign(a) => {
                let scale = self.eval_scale(&a.right);
                if let Expr::Path(p) = a.left.as_ref() {
                    if let Some(ident) = p.path.get_ident() {
                        self.var_scales.insert(ident.to_string(), scale);
                    }
                }
                Scale::Unknown
            }
            Expr::Call(call) => {
                for arg in &call.args {
                    self.eval_scale(arg);
                }
                Scale::Unknown
            }
            Expr::Return(ret) => {
                if let Some(value) = &ret.expr {
                    self.eval_scale(value);
                }
                Scale::Unknown
            }
            Expr::Try(t) => self.eval_scale(&t.expr),
            Expr::Group(g) => self.eval_scale(&g.expr),
            Expr::Block(b) => {
                self.analyze_scoped_block(&b.block);
                Scale::Unknown
            }
            Expr::Unsafe(u) => {
                self.analyze_scoped_block(&u.block);
                Scale::Unknown
            }
            Expr::If(i) => {
                self.eval_scale(&i.cond);
                self.analyze_scoped_block(&i.then_branch);
                if let Some((_, otherwise)) = &i.else_branch {
                    self.eval_scoped(otherwise);
                }
                Scale::Unknown
            }
            Expr::Match(m) => {
                self.eval_scale(&m.expr);
                for arm in &m.arms {
                    let outer = self.var_scales.clone();
                    self.bind_pat(&arm.pat, Scale::Unknown);
                    if let Some((_, guard)) = &arm.guard {
                        self.eval_scale(guard);
                    }
                    self.eval_scale(&arm.body);
                    self.var_scales = outer;
                }
                Scale::Unknown
            }
            Expr::While(w) => {
                self.eval_scale(&w.cond);
                self.analyze_scoped_block(&w.body);
                Scale::Unknown
            }
            Expr::Loop(l) => {
                self.analyze_scoped_block(&l.body);
                Scale::Unknown
            }
            Expr::ForLoop(f) => {
                self.eval_scale(&f.expr);
                let outer = self.var_scales.clone();
                self.bind_pat(&f.pat, Scale::Unknown);
                self.analyze_block(&f.body);
                self.var_scales = outer;
                Scale::Unknown
            }
            Expr::Let(l) => {
                self.eval_scale(&l.expr);
                Scale::Unknown
            }
            Expr::Tuple(t) => {
                for elem in &t.elems {
                    self.eval_scale(elem);
                }
                Scale::Unknown
            }
            Expr::Array(a) => {
                for elem in &a.elems {
                    self.eval_scale(elem);
                }
                Scale::Unknown
            }
            Expr::Struct(s) => {
                for field in &s.fields {
                    self.eval_scale(&field.expr);
                }
                if let Some(rest) = &s.rest {
                    self.eval_scale(rest);
                }
                Scale::Unknown
            }
            Expr::Index(i) => {
                self.eval_scale(&i.expr);
                self.eval_scale(&i.index);
                Scale::Unknown
            }
            Expr::Field(f) => {
                self.eval_scale(&f.base);
                Scale::Unknown
            }
            Expr::Range(r) => {
                if let Some(start) = &r.start {
                    self.eval_scale(start);
                }
                if let Some(end) = &r.end {
                    self.eval_scale(end);
                }
                Scale::Unknown
            }
            Expr::Closure(c) => {
                let outer = self.var_scales.clone();
                for input in &c.inputs {
                    self.bind_pat(input, Scale::Unknown);
                }
                self.eval_scale(&c.body);
                self.var_scales = outer;
                Scale::Unknown
            }
            Expr::MethodCall(mc) => {
                for arg in &mc.args {
                    self.eval_scale(arg);
                }
                self.eval_scale(&mc.receiver)
            }

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
                                        "Naclac Error: decimal scale mismatch: left has {l} \
                                         decimal places, right has {r}. Normalize both to one \
                                         scale before adding or subtracting."
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

#[cfg(test)]
mod tests {
    use {super::ScaleTracker, std::collections::HashMap};

    /// The mismatch count for `body` with `a` at scale 2 and `b` at scale 3.
    fn mismatches(body: &str) -> usize {
        let block: syn::Block = syn::parse_str(body).unwrap();
        let scales = HashMap::from([("a".to_string(), 2), ("b".to_string(), 3)]);
        let mut tracker = ScaleTracker::new(scales);
        tracker.analyze_block(&block);
        tracker.errors.len()
    }

    #[test]
    fn a_mismatch_is_reported_wherever_the_arithmetic_sits() {
        for body in [
            "{ let total = a + b; }",
            "{ let mut total = 0; total = a + b; }",
            "{ Ok(a + b) }",
            "{ return Ok(a + b); }",
            "{ f(1, a - b) }",
            "{ x.checked_add(a + b) }",
            "{ if a > 0 { let t = a + b; } }",
            "{ if a > 0 { a } else { a + b } }",
            "{ match a { 0 => a + b, _ => a } }",
            "{ for i in 0..3 { g(a + b); } }",
            "{ let total = { a + b }; }",
            "{ (a + b, a) }",
        ] {
            assert_eq!(mismatches(body), 1, "{body}");
        }
    }

    #[test]
    fn matching_scales_and_scale_changing_operations_are_accepted() {
        for body in [
            "{ Ok(a + a) }",
            "{ let total = a * b; Ok(total - total) }",
            "{ if a > 0 { a + a } else { b + b } }",
        ] {
            assert_eq!(mismatches(body), 0, "{body}");
        }
    }

    #[test]
    fn a_name_rebound_inside_a_block_keeps_its_outer_scale_after_it() {
        assert_eq!(mismatches("{ { let a = b; } let total = a + a; }"), 0);
        assert_eq!(mismatches("{ { let a = b; let t = a + b; } }"), 0);
    }
}
