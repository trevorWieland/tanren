use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn legacy_install_script_fails_fast_with_migration_guidance() {
    let root = workspace_root();
    let output = Command::new("bash")
        .arg("scripts/install.sh")
        .current_dir(&root)
        .output()
        .expect("run legacy installer");
    assert!(!output.status.success(), "legacy installer must fail-fast");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("permanently deprecated"),
        "expected deprecation messaging: {stderr}"
    );
    assert!(
        stderr.contains("scripts/runtime/install-runtime.sh")
            && stderr.contains("scripts/runtime/verify-installed-runtime.sh")
            && stderr.contains("tanren-cli install"),
        "expected migration guidance in stderr: {stderr}"
    );
}
