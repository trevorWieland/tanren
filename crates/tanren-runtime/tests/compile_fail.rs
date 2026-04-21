use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn assert_compile_fail(test_name: &str, source: &str, stderr_expectations: &[&str]) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let workspace = std::env::temp_dir().join(format!(
        "tanren-runtime-compile-fail-{test_name}-{}-{nanos}",
        std::process::id()
    ));
    let src_dir = workspace.join("src");
    assert!(
        fs::create_dir_all(&src_dir).is_ok(),
        "failed to create temporary compile-fail workspace"
    );

    let manifest = format!(
        "[package]\nname = \"compile-fail-{test_name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\ntanren-runtime = {{ path = \"{}\" }}\n",
        manifest_dir.display()
    );
    assert!(
        fs::write(workspace.join("Cargo.toml"), manifest).is_ok(),
        "failed to write compile-fail Cargo.toml"
    );
    assert!(
        fs::write(src_dir.join("main.rs"), source).is_ok(),
        "failed to write compile-fail source"
    );

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let output = Command::new(cargo)
        .arg("check")
        .arg("--manifest-path")
        .arg(workspace.join("Cargo.toml"))
        .output();
    let _ = fs::remove_dir_all(&workspace);

    assert!(
        output.is_ok(),
        "failed to invoke cargo for compile-fail test"
    );
    let Some(output) = output.ok() else {
        return;
    };

    assert!(
        !output.status.success(),
        "compile-fail fixture `{test_name}` unexpectedly compiled"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr_expectations
            .iter()
            .any(|needle| stderr.contains(needle)),
        "unexpected compiler stderr for `{test_name}`: {stderr}"
    );
}

#[test]
fn harness_failure_new_is_not_public_outside_crate() {
    assert_compile_fail(
        "harness-new",
        "use tanren_runtime::{HarnessFailure, ProviderFailureCode};\n\nfn main() {\n    let _ = HarnessFailure::new(ProviderFailureCode::Fatal, \"unsanitized\");\n}\n",
        &[
            "HarnessFailure::new",
            "private associated function",
            "no function or associated item named `new`",
        ],
    );
}

#[test]
fn contract_call_token_cannot_be_constructed_outside_crate() {
    assert_compile_fail(
        "contract-call-token",
        "use tanren_runtime::ContractCallToken;\n\nfn main() {\n    let _ = ContractCallToken { };\n}\n",
        &[
            "cannot construct",
            "private field",
            "initializer of inaccessible field",
        ],
    );
}
