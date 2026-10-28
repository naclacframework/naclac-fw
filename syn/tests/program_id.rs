//! A crate's address is the one it declares with `declare_id!` in its crate
//! root, and a crate that declares none, several, or a non-literal one is an
//! error that says so.

use {
    naclac_syn::program_id::declared_id,
    std::{fs, path::PathBuf},
};

fn crate_with(tag: &str, lib_rs: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "naclac-syn-program-id-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"sample\"\n").unwrap();
    fs::write(dir.join("src/lib.rs"), lib_rs).unwrap();
    dir
}

fn declared(tag: &str, lib_rs: &str) -> Result<(String, PathBuf), String> {
    let dir = crate_with(tag, lib_rs);
    let result = declared_id(&dir).map(|id| (id.address, id.file));
    let _ = fs::remove_dir_all(&dir);
    result
}

#[test]
fn the_declared_address_and_its_file_are_returned() {
    let (address, file) = declared(
        "found",
        "use naclac_lang::prelude::*;\ndeclare_id!(\"11111111111111111111111111111111\");\n",
    )
    .unwrap();
    assert_eq!(address, "11111111111111111111111111111111");
    assert!(file.ends_with("src/lib.rs"), "{}", file.display());
}

#[test]
fn a_qualified_declare_id_is_found() {
    let (address, _) = declared(
        "qualified",
        "naclac_lang::declare_id!(\"Abc111111111111111111111111111111\");\n",
    )
    .unwrap();
    assert_eq!(address, "Abc111111111111111111111111111111");
}

#[test]
fn a_crate_without_declare_id_is_an_error() {
    let error = declared("missing", "pub fn nothing() {}\n").unwrap_err();
    assert!(
        error.contains("must declare its address with `declare_id!"),
        "{error}"
    );
}

#[test]
fn a_second_declare_id_is_an_error() {
    let error = declared(
        "twice",
        "declare_id!(\"11111111111111111111111111111111\");\n\
         declare_id!(\"22222222222222222222222222222222\");\n",
    )
    .unwrap_err();
    assert!(error.contains("more than once"), "{error}");
}

#[test]
fn an_address_that_is_not_a_string_literal_is_an_error() {
    let error = declared("not-literal", "declare_id!(SOME_CONST);\n").unwrap_err();
    assert!(error.contains("string literal"), "{error}");
}

#[test]
fn a_declare_id_outside_the_crate_root_does_not_count() {
    let error = declared(
        "inner",
        "pub mod inner {\n    declare_id!(\"11111111111111111111111111111111\");\n}\n",
    )
    .unwrap_err();
    assert!(error.contains("must declare its address"), "{error}");
}
