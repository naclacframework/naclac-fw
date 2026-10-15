//! Diagnostics that can't be `trybuild` snapshots: a `trybuild` `.stderr`
//! must match byte for byte, but these errors' notes point into naclac-core
//! by absolute path (machine-specific) or need a real crate (a discoverable
//! program ID in `Naclac.toml`). Each fixture under `../crate-scan-fixtures/`
//! is built with a real `cargo check` and its message text asserted instead.

use std::path::Path;
use std::process::Command;

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
fn event_cpi_without_a_resolvable_program_id_fails_to_compile() {
    let stderr = check_fails("event_cpi_missing_program_id");
    assert!(
        stderr.contains("`#[event_cpi]` needs the program id to derive the event authority PDA"),
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
