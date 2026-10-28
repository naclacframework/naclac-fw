//! Diagnostics that can't be `trybuild` snapshots: a `trybuild` `.stderr`
//! must match byte for byte, but these errors' notes point into naclac-core
//! by absolute path (machine-specific) or need a real crate (one that
//! declares, or fails to declare, its program address). Each fixture under
//! `../crate-scan-fixtures/`
//! is built with a real `cargo check` and its message text asserted instead.

use std::path::Path;
use std::process::Command;

fn check_passes(fixture: &str) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crate-scan-fixtures")
        .join(fixture)
        .join("Cargo.toml");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .arg("check")
        .arg("--manifest-path")
        .arg(&manifest)
        .output()
        .expect("failed to run cargo check");
    assert!(
        output.status.success(),
        "{fixture} must compile, but cargo check failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn check_fails(fixture: &str) -> String {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crate-scan-fixtures")
        .join(fixture)
        .join("Cargo.toml");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .arg("check")
        .arg("--manifest-path")
        .arg(&manifest)
        .output()
        .expect("failed to run cargo check");
    assert!(
        !output.status.success(),
        "{fixture} must fail to compile, but cargo check succeeded"
    );
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn a_vec_field_without_max_len_fails_where_space_is_used() {
    let stderr = check_fails("max_len_missing");
    assert!(
        stderr.contains("`Ledger` has a `Vec`/`String` field `entries` with no `#[max_len(N)]`"),
        "{stderr}"
    );
}

#[test]
fn a_defined_type_with_an_unbounded_field_fails_where_its_size_is_used() {
    let stderr = check_fails("max_len_nested_missing");
    assert!(
        stderr.contains("`Inner` has a `Vec`/`String` field `items` with no `#[max_len(N)]`"),
        "{stderr}"
    );
}

#[test]
fn max_len_on_a_field_with_nothing_to_limit_is_rejected() {
    let stderr = check_fails("max_len_without_collection");
    assert!(
        stderr.contains("has more limits than the field has `Vec` or `String` levels"),
        "{stderr}"
    );
}

#[test]
fn a_type_alias_of_string_takes_the_max_len_like_string() {
    check_passes("max_len_on_alias");
}

#[test]
fn a_struct_without_defined_type_has_no_known_maximum_size() {
    let stderr = check_fails("max_len_unknown_type");
    assert!(
        stderr.contains("has no known maximum serialized size"),
        "{stderr}"
    );
}

#[test]
fn emit_cpi_without_event_cpi_names_the_missing_attribute() {
    let stderr = check_fails("emit_cpi_without_event_cpi");
    assert!(
        stderr.contains("`emit_cpi!` needs the handler's accounts struct to carry `#[event_cpi]`"),
        "{stderr}"
    );
    assert!(
        stderr.contains("`Plain` is not annotated with `#[event_cpi]`"),
        "{stderr}"
    );
}

#[test]
fn a_program_without_declare_id_fails_to_compile() {
    let stderr = check_fails("event_cpi_missing_program_id");
    assert!(
        stderr.contains("the program must declare its address with `declare_id!"),
        "{stderr}"
    );
}

#[test]
fn an_address_that_is_not_canonical_base58_fails_to_compile() {
    let stderr = check_fails("declare_id_not_canonical");
    assert!(
        stderr.contains("not the canonical base58 encoding of 32 bytes"),
        "{stderr}"
    );
}

#[test]
fn event_cpi_below_the_derive_names_the_required_order() {
    let stderr = check_fails("event_cpi_wrong_order");
    assert!(
        stderr.contains("#[event_cpi] must be written directly above #[derive(Accounts)]"),
        "{stderr}"
    );
}
