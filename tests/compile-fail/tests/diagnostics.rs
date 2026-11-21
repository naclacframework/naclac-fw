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
        stderr.contains(
            "`Ledger.entries` is Vec/String with no `#[max_len(N)]`, so its size is unknown."
        ),
        "{stderr}"
    );
}

#[test]
fn a_defined_type_with_an_unbounded_field_fails_where_its_size_is_used() {
    let stderr = check_fails("max_len_nested_missing");
    assert!(
        stderr.contains(
            "`Inner.items` is Vec/String with no `#[max_len(N)]`, so its maximum size is unknown."
        ),
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
fn a_float_behind_an_alias_is_rejected_by_system() {
    let stderr = check_fails("alias_float_in_system");
    assert!(
        stderr.contains("this type is `f32`/`f64`, which isn't allowed in #[system]"),
        "{stderr}"
    );
}

#[test]
fn an_alias_of_account_info_needs_a_safety_doc() {
    let stderr = check_fails("alias_account_info_without_safety_doc");
    assert!(
        stderr.contains("this field's type gets no automatic validation"),
        "{stderr}"
    );
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
        stderr.contains("cannot find value `ID` in the crate root"),
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

#[test]
fn an_all_zero_account_discriminator_is_rejected() {
    let stderr = check_fails("zero_account_disc");
    assert!(
        stderr.contains("has an all-zero discriminator, which marks an uninitialized account."),
        "{stderr}"
    );
}

#[test]
fn an_instruction_using_the_event_cpi_tag_is_rejected_while_event_cpi_is_in_use() {
    let stderr = check_fails("event_cpi_tag_collision");
    assert!(
        stderr.contains("use the discriminator reserved for event-CPI"),
        "{stderr}"
    );
}

#[test]
fn the_same_shape_with_the_default_discriminator_compiles() {
    check_passes("event_cpi_no_collision");
}

fn idl_step_passes(fixture: &str) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crate-scan-fixtures")
        .join(fixture)
        .join("Cargo.toml");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .arg("test")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--features")
        .arg("idl-build")
        .output()
        .expect("failed to run cargo test");
    assert!(
        output.status.success(),
        "{fixture}'s idl-build test must pass:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn two_accounts_with_one_discriminator_fail_the_idl_step() {
    idl_step_passes("dup_account_disc");
}

#[test]
fn distinct_account_discriminators_assemble_an_idl() {
    idl_step_passes("distinct_account_disc");
}

#[test]
fn two_items_with_one_name_or_error_code_fail_the_idl_step() {
    idl_step_passes("idl_name_and_code_clashes");
}

#[test]
fn an_opt_of_a_type_the_idl_cannot_name_fails_the_idl_step() {
    idl_step_passes("idl_opt_bad_inner");
}

#[test]
fn a_constant_of_a_type_with_no_idl_text_fails_the_idl_step() {
    idl_step_passes("idl_constant_bad_type");
}

#[test]
fn a_user_type_named_like_an_opt_fails_the_idl_step() {
    idl_step_passes("idl_opt_name_clash");
}

fn build_fails_with_features(features: &str) -> String {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crate-scan-fixtures")
        .join("config_conflicts")
        .join("Cargo.toml");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut command = Command::new(cargo);
    command
        .arg("check")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--no-default-features");
    if !features.is_empty() {
        command.arg("--features").arg(features);
    }
    let output = command.output().expect("failed to run cargo check");
    assert!(
        !output.status.success(),
        "features `{features}` must fail to build, but cargo check succeeded"
    );
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_build_error(features: &str, expected: &str) {
    let stderr = build_fails_with_features(features);
    assert!(stderr.contains(expected), "{features}: {stderr}");
}

#[test]
fn a_build_with_no_runtime_is_rejected() {
    assert_build_error("", "naclac-core: select a runtime");
}

#[test]
fn borsh_and_wincode_together_are_rejected() {
    assert_build_error(
        "solana,borsh,wincode",
        "the `borsh` and `wincode` features are mutually exclusive",
    );
}

#[test]
fn bytemuck_and_borsh_together_are_rejected() {
    assert_build_error(
        "solana,bytemuck,borsh",
        "the `bytemuck` and `borsh` features are mutually exclusive",
    );
}

#[test]
fn bytemuck_and_wincode_together_are_rejected() {
    assert_build_error(
        "solana,bytemuck,wincode",
        "the `bytemuck` and `wincode` features are mutually exclusive",
    );
}

#[test]
fn borsh_on_the_pinocchio_runtime_is_rejected() {
    assert_build_error(
        "pinocchio,borsh",
        "the `borsh` feature is not available with the `pinocchio` runtime",
    );
}

#[test]
fn both_runtimes_together_are_rejected() {
    assert_build_error(
        "solana,pinocchio",
        "naclac-lang: the `solana` and `pinocchio` runtimes are mutually exclusive",
    );
}

#[test]
fn the_token_feature_without_a_runtime_is_rejected() {
    assert_build_error(
        "token",
        "the `token` feature requires `solana` or `pinocchio` to also be enabled",
    );
}

#[test]
fn the_metadata_feature_without_a_runtime_is_rejected() {
    assert_build_error(
        "metadata",
        "the `metadata` feature requires `solana` or `pinocchio` to also be enabled",
    );
}
