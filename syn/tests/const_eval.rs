//! Pins `naclac_syn::const_eval` to rustc's constant semantics and to the
//! `Debug` text the compiled `idl-build` path prints for the same constant.

use naclac_syn::const_eval::{evaluate_constant, ConstEnv, EvalError};

fn env_of(source: &str) -> ConstEnv {
    let mut env = ConstEnv::default();
    env.collect(&syn::parse_file(source).unwrap().items);
    env
}

fn eval(ty: &str, expr: &str) -> Result<Option<String>, EvalError> {
    evaluate_constant(
        &syn::parse_str(ty).unwrap(),
        &syn::parse_str(expr).unwrap(),
        &ConstEnv::default(),
    )
}

fn value(ty: &str, expr: &str) -> String {
    eval(ty, expr).unwrap().unwrap()
}

#[test]
fn literals_render_like_debug() {
    assert_eq!(value("u64", "10_000"), "10000");
    assert_eq!(value("u32", "0xFF"), "255");
    assert_eq!(value("u32", "7u32"), "7");
    assert_eq!(value("i32", "-5"), "-5");
    assert_eq!(value("i8", "-128"), "-128");
    assert_eq!(value("bool", "true"), "true");
    assert_eq!(value("f64", "1.5"), "1.5");
    assert_eq!(value("f64", "1e3"), "1000.0");
    assert_eq!(value("f64", "2.0"), "2.0");
    assert_eq!(value("f32", "0.1"), "0.1");
}

#[test]
fn arithmetic_and_bit_operators() {
    assert_eq!(value("u64", "2 * 5"), "10");
    assert_eq!(value("u64", "(1 + 2) * 3 - 4"), "5");
    assert_eq!(value("u64", "17 / 5"), "3");
    assert_eq!(value("i64", "-17 / 5"), "-3");
    assert_eq!(value("i64", "-17 % 5"), "-2");
    assert_eq!(value("u8", "0b1010 & 0b0110"), "2");
    assert_eq!(value("u8", "0b1010 | 0b0110"), "14");
    assert_eq!(value("u8", "0b1010 ^ 0b0110"), "12");
    assert_eq!(value("u64", "1 << 20"), "1048576");
    assert_eq!(value("u64", "1 << 20 >> 4"), "65536");
    assert_eq!(value("u8", "0xFF << 1"), "254");
    assert_eq!(value("u8", "!0b1010"), "245");
    assert_eq!(value("i8", "!0"), "-1");
    assert_eq!(value("bool", "1 < 2 && 3 >= 3"), "true");
    assert_eq!(value("f64", "1.5 * 4.0"), "6.0");
    assert_eq!(value("u64", "10u64.pow(9)"), "1000000000");
}

#[test]
fn casts_follow_rustc() {
    assert_eq!(value("u8", "300 as u8"), "44");
    assert_eq!(value("i8", "200 as i8"), "-56");
    assert_eq!(value("u64", "-1i32 as u64"), "18446744073709551615");
    assert_eq!(value("f64", "3 as f64"), "3.0");
    assert_eq!(value("u8", "-5.5f64 as u8"), "0");
    assert_eq!(value("u8", "300.7f64 as u8"), "255");
    assert_eq!(value("i32", "2.9f64 as i32"), "2");
}

#[test]
fn associated_constants_and_sizes() {
    assert_eq!(value("u64", "u64::MAX"), "18446744073709551615");
    assert_eq!(value("i64", "i64::MIN"), "-9223372036854775808");
    assert_eq!(value("u32", "u8::MAX as u32 + 1"), "256");
    assert_eq!(value("usize", "core::mem::size_of::<u64>()"), "8");
    assert_eq!(value("usize", "size_of::<u16>() * 4"), "8");
    assert_eq!(value("u32", "u16::BITS"), "16");
}

#[test]
fn constants_refer_to_each_other() {
    let env = env_of(
        r#"
        pub const BASE: u64 = 1_000;
        pub const SCALE: u64 = BASE * 3;
        pub const MASK: u8 = (1 << 4) - 1;
        pub const LOOP_A: u64 = LOOP_B;
        pub const LOOP_B: u64 = LOOP_A;
        "#,
    );
    let run = |ty: &str, expr: &str| {
        evaluate_constant(
            &syn::parse_str(ty).unwrap(),
            &syn::parse_str(expr).unwrap(),
            &env,
        )
    };
    assert_eq!(run("u64", "SCALE + BASE").unwrap().unwrap(), "4000");
    assert_eq!(run("u8", "MASK").unwrap().unwrap(), "15");
    assert!(matches!(
        run("u64", "LOOP_A"),
        Err(EvalError::Unsupported(_))
    ));
    assert!(matches!(
        run("u64", "UNKNOWN + 1"),
        Err(EvalError::Unsupported(_))
    ));
}

#[test]
fn arrays_render_like_debug() {
    assert_eq!(value("[u8; 3]", "[1, 2, 3]"), "[1, 2, 3]");
    assert_eq!(value("[u16; 3]", "[1, 1 + 1, 3 * 2]"), "[1, 2, 6]");
    assert_eq!(value("[u8; 4]", "[7; 4]"), "[7, 7, 7, 7]");
    assert_eq!(value("&[u32]", "&[1, 2]"), "[1, 2]");
    assert_eq!(value("[u8; 2]", "*&[1, 2]"), "[1, 2]");
}

#[test]
fn rustc_rejects_these_so_they_are_invalid() {
    for (ty, expr) in [
        ("u8", "255 + 1"),
        ("u8", "256"),
        ("u64", "0 - 1"),
        ("u64", "1 / 0"),
        ("u32", "1 << 32"),
        ("u64", "10u64.pow(30)"),
        ("i8", "-129"),
    ] {
        assert!(
            matches!(eval(ty, expr), Err(EvalError::Invalid(_))),
            "{ty} = {expr}: {:?}",
            eval(ty, expr)
        );
    }
}

#[test]
fn what_it_cannot_do_is_reported_not_guessed() {
    for (ty, expr) in [
        ("u64", "compute()"),
        ("u64", "if true { 1 } else { 2 }"),
        ("u128", "u128::MAX"),
        ("u64", "SOME.len()"),
    ] {
        assert!(
            matches!(eval(ty, expr), Err(EvalError::Unsupported(_))),
            "{ty} = {expr}: {:?}",
            eval(ty, expr)
        );
    }
    assert!(eval("&str", "\"text\"").unwrap().is_none());
    assert!(
        eval("Address", "address!(\"11111111111111111111111111111111\")")
            .unwrap()
            .is_none()
    );
}
