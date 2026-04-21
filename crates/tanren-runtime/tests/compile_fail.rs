use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn harness_failure_new_is_not_public_outside_crate() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let workspace = std::env::temp_dir().join(format!(
        "tanren-runtime-compile-fail-{}-{nanos}",
        std::process::id()
    ));
    let src_dir = workspace.join("src");
    assert!(
        fs::create_dir_all(&src_dir).is_ok(),
        "failed to create temporary compile-fail workspace"
    );

    let manifest = format!(
        "[package]\nname = \"compile-fail-harness-new\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\ntanren-runtime = {{ path = \"{}\" }}\n",
        manifest_dir.display()
    );
    assert!(
        fs::write(workspace.join("Cargo.toml"), manifest).is_ok(),
        "failed to write compile-fail Cargo.toml"
    );
    assert!(
        fs::write(
            src_dir.join("main.rs"),
            "use tanren_runtime::{HarnessFailure, ProviderFailureCode};\n\nfn main() {\n    let _ = HarnessFailure::new(ProviderFailureCode::Fatal, \"unsanitized\");\n}\n",
        )
        .is_ok(),
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
        "calling HarnessFailure::new outside the crate must not compile"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("HarnessFailure::new")
            || stderr.contains("private associated function")
            || stderr.contains("no function or associated item named `new`"),
        "unexpected compiler stderr: {stderr}"
    );
}
