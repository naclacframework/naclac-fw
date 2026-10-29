//! Evaluation of `const` initializer expressions without a compiler, for the
//! array lengths and constant references the macros resolve at expansion
//! time. The result is the constant's `Debug` text, byte for byte what the
//! compiled `idl-build` path prints for the same constant.
//!
//! Supported: integer, float and bool literals with their suffixes; unary
//! `-` and `!`; binary `+ - * / % & | ^ << >>`, comparisons and `&& ||`;
//! parentheses; `as` casts between primitives; other constants of the crate;
//! `T::MAX`, `T::MIN`, `T::BITS`; `size_of::<T>()` and `align_of::<T>()` of a
//! primitive; `.pow(n)`; arrays and `[x; N]` of those. Arithmetic follows
//! rustc's const evaluation: overflow and division by zero are errors, and
//! sizes are those of the 64-bit host the `idl-build` compile runs on. A `u128`
//! value above `i128::MAX` is not representable here and reports as
//! unsupported.

use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ty {
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
    F32,
    F64,
    Bool,
}

impl Ty {
    fn from_name(name: &str) -> Option<Ty> {
        Some(match name {
            "i8" => Ty::I8,
            "i16" => Ty::I16,
            "i32" => Ty::I32,
            "i64" => Ty::I64,
            "i128" => Ty::I128,
            "isize" => Ty::Isize,
            "u8" => Ty::U8,
            "u16" => Ty::U16,
            "u32" => Ty::U32,
            "u64" => Ty::U64,
            "u128" => Ty::U128,
            "usize" => Ty::Usize,
            "f32" => Ty::F32,
            "f64" => Ty::F64,
            "bool" => Ty::Bool,
            _ => return None,
        })
    }

    fn from_type(ty: &syn::Type) -> Option<Ty> {
        match ty {
            syn::Type::Path(p) if p.qself.is_none() && p.path.segments.len() == 1 => {
                Ty::from_name(&p.path.segments[0].ident.to_string())
            }
            _ => None,
        }
    }

    fn is_float(self) -> bool {
        matches!(self, Ty::F32 | Ty::F64)
    }

    fn is_int(self) -> bool {
        !self.is_float() && self != Ty::Bool
    }

    fn bits(self) -> u32 {
        match self {
            Ty::I8 | Ty::U8 | Ty::Bool => 8,
            Ty::I16 | Ty::U16 => 16,
            Ty::I32 | Ty::U32 | Ty::F32 => 32,
            Ty::I64 | Ty::U64 | Ty::Isize | Ty::Usize | Ty::F64 => 64,
            Ty::I128 | Ty::U128 => 128,
        }
    }

    fn signed(self) -> bool {
        matches!(
            self,
            Ty::I8 | Ty::I16 | Ty::I32 | Ty::I64 | Ty::I128 | Ty::Isize
        )
    }

    fn min(self) -> i128 {
        if self.signed() {
            if self.bits() == 128 {
                i128::MIN
            } else {
                -(1i128 << (self.bits() - 1))
            }
        } else {
            0
        }
    }

    fn max(self) -> Option<i128> {
        match (self.signed(), self.bits()) {
            (true, 128) => Some(i128::MAX),
            (true, b) => Some((1i128 << (b - 1)) - 1),
            (false, 128) => None,
            (false, b) => Some((1i128 << b) - 1),
        }
    }

    fn fits(self, value: i128) -> bool {
        value >= self.min() && self.max().is_none_or(|max| value <= max)
    }
}

#[derive(Clone, Copy, Debug)]
enum Val {
    Int(i128, Ty),
    Float(f64, Ty),
    Bool(bool),
}

/// Why a constant has no evaluated value.
#[derive(Debug)]
pub enum EvalError {
    /// Rustc would reject the constant (overflow, division by zero, a
    /// literal out of range).
    Invalid(String),
    /// The evaluator does not support the expression; only compiling the
    /// program can produce the value.
    Unsupported(String),
}

fn unsupported<T>(what: impl Into<String>) -> Result<T, EvalError> {
    Err(EvalError::Unsupported(what.into()))
}

fn invalid<T>(what: impl Into<String>) -> Result<T, EvalError> {
    Err(EvalError::Invalid(what.into()))
}

/// The crate's constants, by name, for constants that refer to each other.
#[derive(Default)]
pub struct ConstEnv {
    consts: HashMap<String, (syn::Type, syn::Expr)>,
}

impl ConstEnv {
    /// Records every `const` among one module's `items`.
    pub fn collect(&mut self, items: &[syn::Item]) {
        for item in items {
            if let syn::Item::Const(c) = item {
                self.consts
                    .insert(c.ident.to_string(), ((*c.ty).clone(), (*c.expr).clone()));
            }
        }
    }
}

const MAX_DEPTH: usize = 64;

/// The `Debug` text of the constant `expr` of declared type `ty`.
/// `Ok(None)` when `ty` is not a type this evaluator handles (a scalar, or an
/// array or slice reference of scalars); the caller keeps its own rendering.
pub fn evaluate_constant(
    ty: &syn::Type,
    expr: &syn::Expr,
    env: &ConstEnv,
) -> Result<Option<String>, EvalError> {
    if let Some(scalar) = Ty::from_type(ty) {
        return Ok(Some(format(eval(expr, Some(scalar), env, 0)?)));
    }
    let array_ty = match ty {
        syn::Type::Reference(r) => &*r.elem,
        other => other,
    };
    let elem_ty = match array_ty {
        syn::Type::Array(a) => Ty::from_type(&a.elem),
        syn::Type::Slice(s) => Ty::from_type(&s.elem),
        _ => None,
    };
    let Some(elem_ty) = elem_ty else {
        return Ok(None);
    };
    let values = eval_elements(expr, elem_ty, env, 0)?;
    Ok(Some(format!(
        "[{}]",
        values
            .into_iter()
            .map(format)
            .collect::<Vec<_>>()
            .join(", ")
    )))
}

fn eval_elements(
    expr: &syn::Expr,
    elem_ty: Ty,
    env: &ConstEnv,
    depth: usize,
) -> Result<Vec<Val>, EvalError> {
    match expr {
        syn::Expr::Reference(r) => eval_elements(&r.expr, elem_ty, env, depth),
        syn::Expr::Paren(p) => eval_elements(&p.expr, elem_ty, env, depth),
        syn::Expr::Group(g) => eval_elements(&g.expr, elem_ty, env, depth),
        syn::Expr::Unary(u) if matches!(u.op, syn::UnOp::Deref(_)) => {
            eval_elements(&u.expr, elem_ty, env, depth)
        }
        syn::Expr::Array(a) => a
            .elems
            .iter()
            .map(|e| eval(e, Some(elem_ty), env, depth))
            .collect(),
        syn::Expr::Repeat(r) => {
            let element = eval(&r.expr, Some(elem_ty), env, depth)?;
            let Val::Int(len, _) = eval(&r.len, Some(Ty::Usize), env, depth)? else {
                return unsupported("array length is not an integer");
            };
            Ok(vec![
                element;
                usize::try_from(len)
                    .or_else(|_| invalid("array length"))?
            ])
        }
        syn::Expr::Path(_) => {
            let Some((ty, expr)) = referenced_const(expr, env) else {
                return unsupported("array constant refers to something that is not a constant");
            };
            let array_elem = match &ty {
                syn::Type::Array(a) => Ty::from_type(&a.elem),
                syn::Type::Reference(r) => match &*r.elem {
                    syn::Type::Array(a) => Ty::from_type(&a.elem),
                    syn::Type::Slice(s) => Ty::from_type(&s.elem),
                    _ => None,
                },
                _ => None,
            };
            match array_elem {
                Some(t) if depth < MAX_DEPTH => eval_elements(&expr, t, env, depth + 1),
                _ => unsupported("array constant of an unsupported type"),
            }
        }
        _ => unsupported("array expression"),
    }
}

fn format(value: Val) -> String {
    match value {
        Val::Int(v, _) => v.to_string(),
        Val::Float(v, Ty::F32) => format!("{:?}", v as f32),
        Val::Float(v, _) => format!("{v:?}"),
        Val::Bool(v) => v.to_string(),
    }
}

fn referenced_const(expr: &syn::Expr, env: &ConstEnv) -> Option<(syn::Type, syn::Expr)> {
    let syn::Expr::Path(p) = expr else {
        return None;
    };
    let last = p.path.segments.last()?;
    env.consts.get(&last.ident.to_string()).cloned()
}

fn int_in_range(value: i128, ty: Ty) -> Result<Val, EvalError> {
    if ty.fits(value) {
        Ok(Val::Int(value, ty))
    } else {
        invalid(format!("value {value} does not fit in {ty:?}"))
    }
}

fn float_in(value: f64, ty: Ty) -> Val {
    Val::Float(
        if ty == Ty::F32 {
            f64::from(value as f32)
        } else {
            value
        },
        ty,
    )
}

fn eval(
    expr: &syn::Expr,
    expected: Option<Ty>,
    env: &ConstEnv,
    depth: usize,
) -> Result<Val, EvalError> {
    if depth > MAX_DEPTH {
        return unsupported("constants refer to each other too deeply");
    }
    match expr {
        syn::Expr::Paren(p) => eval(&p.expr, expected, env, depth),
        syn::Expr::Group(g) => eval(&g.expr, expected, env, depth),
        syn::Expr::Lit(l) => eval_literal(&l.lit, expected, false),
        syn::Expr::Unary(u) => match u.op {
            syn::UnOp::Neg(_) => match &*u.expr {
                syn::Expr::Lit(l) if matches!(l.lit, syn::Lit::Int(_)) => {
                    eval_literal(&l.lit, expected, true)
                }
                operand => match eval(operand, expected, env, depth)? {
                    Val::Int(v, ty) if ty.signed() => int_in_range(-v, ty),
                    Val::Float(v, ty) => Ok(Val::Float(-v, ty)),
                    _ => invalid("`-` applied to an unsigned integer or a bool"),
                },
            },
            syn::UnOp::Not(_) => match eval(&u.expr, expected, env, depth)? {
                Val::Bool(b) => Ok(Val::Bool(!b)),
                Val::Int(v, ty) if ty.signed() => Ok(Val::Int(!v, ty)),
                Val::Int(v, ty) => match ty.max() {
                    Some(max) => Ok(Val::Int(max - v, ty)),
                    None => unsupported("`!` on a u128"),
                },
                Val::Float(..) => invalid("`!` applied to a float"),
            },
            _ => unsupported("dereference in a scalar constant"),
        },
        syn::Expr::Binary(b) => eval_binary(b, expected, env, depth),
        syn::Expr::Cast(c) => {
            let Some(target) = Ty::from_type(&c.ty) else {
                return unsupported("cast to a non-primitive type");
            };
            cast(eval(&c.expr, None, env, depth)?, target)
        }
        syn::Expr::Path(p) => eval_path(expr, p, env, depth),
        syn::Expr::Call(call) => eval_call(call),
        syn::Expr::MethodCall(m) if m.method == "pow" && m.args.len() == 1 => {
            let base = eval(&m.receiver, expected, env, depth)?;
            let Val::Int(exp, _) = eval(&m.args[0], Some(Ty::U32), env, depth)? else {
                return unsupported("`pow` exponent");
            };
            let Val::Int(base, ty) = base else {
                return unsupported("`pow` on a non-integer");
            };
            let exp = u32::try_from(exp).or_else(|_| invalid("`pow` exponent"))?;
            let value = base
                .checked_pow(exp)
                .ok_or_else(|| EvalError::Invalid("`pow` overflows".into()))?;
            int_in_range(value, ty)
        }
        _ => unsupported("expression kind"),
    }
}

fn eval_literal(lit: &syn::Lit, expected: Option<Ty>, negated: bool) -> Result<Val, EvalError> {
    match lit {
        syn::Lit::Bool(b) => Ok(Val::Bool(b.value)),
        syn::Lit::Int(i) => {
            let ty = match i.suffix() {
                "" => match expected {
                    Some(t) if t.is_int() || t.is_float() => t,
                    _ => Ty::I32,
                },
                suffix => Ty::from_name(suffix)
                    .ok_or_else(|| EvalError::Unsupported(format!("literal suffix `{suffix}`")))?,
            };
            let value: u128 = i
                .base10_parse()
                .or_else(|_| invalid("integer literal does not fit in 128 bits"))?;
            let value =
                i128::try_from(value).or_else(|_| unsupported("a u128 literal above i128::MAX"))?;
            let value = if negated { -value } else { value };
            if ty.is_float() {
                return Ok(float_in(value as f64, ty));
            }
            int_in_range(value, ty)
        }
        syn::Lit::Float(f) => {
            let ty = match f.suffix() {
                "f32" => Ty::F32,
                "f64" => Ty::F64,
                _ => match expected {
                    Some(t) if t.is_float() => t,
                    _ => Ty::F64,
                },
            };
            let value: f64 = f
                .base10_digits()
                .parse()
                .or_else(|_| invalid("float literal"))?;
            Ok(float_in(value, ty))
        }
        _ => unsupported("literal kind"),
    }
}

fn cast(value: Val, target: Ty) -> Result<Val, EvalError> {
    Ok(match (value, target) {
        (Val::Bool(b), t) if t.is_int() => Val::Int(i128::from(b), t),
        (Val::Int(v, _), t) if t.is_int() => {
            let bits = t.bits();
            if bits == 128 {
                return if t.fits(v) {
                    Ok(Val::Int(v, t))
                } else {
                    unsupported("128-bit wrapping cast")
                };
            }
            let modulus = 1i128 << bits;
            let wrapped = v.rem_euclid(modulus);
            Val::Int(
                if t.signed() && wrapped >= modulus / 2 {
                    wrapped - modulus
                } else {
                    wrapped
                },
                t,
            )
        }
        (Val::Int(v, _), t) if t.is_float() => float_in(v as f64, t),
        (Val::Float(v, _), t) if t.is_float() => float_in(v, t),
        (Val::Float(v, _), t) if t.is_int() => {
            let truncated = if v.is_nan() { 0.0 } else { v.trunc() };
            let low = t.min() as f64;
            let high = t.max().map_or(f64::INFINITY, |m| m as f64);
            let clamped = truncated.clamp(low, high);
            match t.max() {
                Some(max) if clamped >= max as f64 => Val::Int(max, t),
                _ if clamped <= low => Val::Int(t.min(), t),
                _ => Val::Int(clamped as i128, t),
            }
        }
        _ => return unsupported("cast"),
    })
}

fn eval_path(
    expr: &syn::Expr,
    path: &syn::ExprPath,
    env: &ConstEnv,
    depth: usize,
) -> Result<Val, EvalError> {
    let segments = &path.path.segments;
    if segments.len() == 2 {
        if let Some(ty) = Ty::from_name(&segments[0].ident.to_string()) {
            return match segments[1].ident.to_string().as_str() {
                "MAX" if ty.is_int() => match ty.max() {
                    Some(max) => Ok(Val::Int(max, ty)),
                    None => unsupported("u128::MAX"),
                },
                "MIN" if ty.is_int() => Ok(Val::Int(ty.min(), ty)),
                "BITS" if ty.is_int() => Ok(Val::Int(i128::from(ty.bits()), Ty::U32)),
                _ => unsupported("associated constant of a primitive"),
            };
        }
    }
    let Some((ty, init)) = referenced_const(expr, env) else {
        return unsupported("a name that is not a constant of this crate");
    };
    let Some(scalar) = Ty::from_type(&ty) else {
        return unsupported("a constant of a non-scalar type used in a scalar expression");
    };
    eval(&init, Some(scalar), env, depth + 1)
}

fn eval_call(call: &syn::ExprCall) -> Result<Val, EvalError> {
    let syn::Expr::Path(func) = &*call.func else {
        return unsupported("call");
    };
    let Some(last) = func.path.segments.last() else {
        return unsupported("call");
    };
    let name = last.ident.to_string();
    if !(name == "size_of" || name == "align_of") || !call.args.is_empty() {
        return unsupported(format!("call to `{name}`"));
    }
    let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
        return unsupported(format!("`{name}` without a type"));
    };
    let Some(syn::GenericArgument::Type(arg)) = args.args.first() else {
        return unsupported(format!("`{name}` without a type"));
    };
    let Some(ty) = Ty::from_type(arg) else {
        return unsupported(format!("`{name}` of a non-primitive type"));
    };
    Ok(Val::Int(i128::from(ty.bits() / 8), Ty::Usize))
}

fn eval_binary(
    b: &syn::ExprBinary,
    expected: Option<Ty>,
    env: &ConstEnv,
    depth: usize,
) -> Result<Val, EvalError> {
    use syn::BinOp::*;
    match b.op {
        And(_) | Or(_) => {
            let (Val::Bool(l), Val::Bool(r)) = (
                eval(&b.left, Some(Ty::Bool), env, depth)?,
                eval(&b.right, Some(Ty::Bool), env, depth)?,
            ) else {
                return invalid("`&&`/`||` on non-bool operands");
            };
            Ok(Val::Bool(if matches!(b.op, And(_)) {
                l && r
            } else {
                l || r
            }))
        }
        Eq(_) | Ne(_) | Lt(_) | Le(_) | Gt(_) | Ge(_) => {
            let left = eval(&b.left, None, env, depth)?;
            let operand_ty = match left {
                Val::Int(_, t) | Val::Float(_, t) => Some(t),
                Val::Bool(_) => Some(Ty::Bool),
            };
            let right = eval(&b.right, operand_ty, env, depth)?;
            let ordering = match (left, right) {
                (Val::Int(l, _), Val::Int(r, _)) => l.partial_cmp(&r),
                (Val::Float(l, _), Val::Float(r, _)) => l.partial_cmp(&r),
                (Val::Bool(l), Val::Bool(r)) => l.partial_cmp(&r),
                _ => return invalid("comparison of different types"),
            };
            Ok(Val::Bool(match (b.op, ordering) {
                (Eq(_), o) => o == Some(std::cmp::Ordering::Equal),
                (Ne(_), o) => o != Some(std::cmp::Ordering::Equal),
                (Lt(_), o) => o == Some(std::cmp::Ordering::Less),
                (Le(_), o) => matches!(
                    o,
                    Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
                ),
                (Gt(_), o) => o == Some(std::cmp::Ordering::Greater),
                (_, o) => matches!(
                    o,
                    Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
                ),
            }))
        }
        Shl(_) | Shr(_) => {
            let Val::Int(l, ty) = eval(&b.left, expected, env, depth)? else {
                return invalid("shift of a non-integer");
            };
            let Val::Int(r, _) = eval(&b.right, None, env, depth)? else {
                return invalid("shift by a non-integer");
            };
            let amount = u32::try_from(r)
                .ok()
                .filter(|a| *a < ty.bits())
                .ok_or_else(|| EvalError::Invalid(format!("shift by {r} overflows {ty:?}")))?;
            if matches!(b.op, Shl(_)) {
                Ok(Val::Int(wrap_to(l.wrapping_shl(amount), ty), ty))
            } else {
                Ok(Val::Int(l >> amount, ty))
            }
        }
        Add(_) | Sub(_) | Mul(_) | Div(_) | Rem(_) | BitAnd(_) | BitOr(_) | BitXor(_) => {
            let left = eval(&b.left, expected, env, depth)?;
            let operand_ty = match left {
                Val::Int(_, t) | Val::Float(_, t) => Some(t),
                Val::Bool(_) => Some(Ty::Bool),
            };
            let right = eval(&b.right, operand_ty, env, depth)?;
            match (left, right) {
                (Val::Int(l, ty), Val::Int(r, _)) => int_binary(b.op, l, r, ty),
                (Val::Float(l, ty), Val::Float(r, _)) => Ok(float_in(
                    match b.op {
                        Add(_) => l + r,
                        Sub(_) => l - r,
                        Mul(_) => l * r,
                        Div(_) => l / r,
                        Rem(_) => l % r,
                        _ => return invalid("bit operation on floats"),
                    },
                    ty,
                )),
                (Val::Bool(l), Val::Bool(r)) => match b.op {
                    BitAnd(_) => Ok(Val::Bool(l & r)),
                    BitOr(_) => Ok(Val::Bool(l | r)),
                    BitXor(_) => Ok(Val::Bool(l ^ r)),
                    _ => invalid("arithmetic on bools"),
                },
                _ => invalid("operands of different types"),
            }
        }
        _ => unsupported("operator"),
    }
}

fn wrap_to(value: i128, ty: Ty) -> i128 {
    if ty.bits() == 128 {
        return value;
    }
    let modulus = 1i128 << ty.bits();
    let wrapped = value.rem_euclid(modulus);
    if ty.signed() && wrapped >= modulus / 2 {
        wrapped - modulus
    } else {
        wrapped
    }
}

fn int_binary(op: syn::BinOp, l: i128, r: i128, ty: Ty) -> Result<Val, EvalError> {
    use syn::BinOp::*;
    let overflow = || EvalError::Invalid(format!("arithmetic overflows {ty:?}"));
    let value = match op {
        Add(_) => l.checked_add(r).ok_or_else(overflow)?,
        Sub(_) => l.checked_sub(r).ok_or_else(overflow)?,
        Mul(_) => l.checked_mul(r).ok_or_else(overflow)?,
        Div(_) | Rem(_) if r == 0 => return invalid("division by zero"),
        Div(_) => l.checked_div(r).ok_or_else(overflow)?,
        Rem(_) => l.checked_rem(r).ok_or_else(overflow)?,
        BitAnd(_) => l & r,
        BitOr(_) => l | r,
        BitXor(_) => l ^ r,
        _ => return unsupported("operator"),
    };
    int_in_range(value, ty)
}
