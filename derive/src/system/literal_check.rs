//! Phase 1b: evaluates constant expressions and rejects literal overflow and division by zero.

use syn::{visit::Visit, BinOp, Error, Expr, ExprBinary, ExprLit, Lit};

pub(super) struct LiteralCheckVisitor {
    pub(super) errors: Vec<Error>,
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
                        "Naclac Error: division or remainder by a literal zero panics at \
                         runtime; remove or guard it.",
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
                            "Naclac Error: constant expression `{l} {sym} {r}` overflows or is \
                             undefined at compile time."
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
