//! Phase 1c: counts arithmetic operations to estimate the function's worst-case Compute Unit cost.

use syn::{visit::Visit, BinOp, ExprBinary};

#[derive(Default)]
pub(super) struct CuEstimator {
    add: usize,
    sub: usize,
    mul: usize,
    div: usize,
    rem: usize,
}

impl CuEstimator {
    // Approximate SBF CU weights per operation type.
    pub(super) fn total_cu(&self) -> usize {
        self.add + self.sub + self.mul * 5 + self.div * 10 + self.rem * 10
    }

    pub(super) fn summary(&self) -> String {
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
