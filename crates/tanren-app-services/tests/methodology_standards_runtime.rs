use std::fs;

use tanren_app_services::methodology::standards::load_runtime_standards;

#[test]
fn missing_root_falls_back_to_builtin_baseline() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("does-not-exist");
    let loaded = load_runtime_standards(&root).expect("fallback");
    assert!(
        !loaded.is_empty(),
        "fallback baseline standards must remain available"
    );
}

#[test]
fn configured_root_loads_and_validates_standard_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("standards");
    let cat = root.join("rust-style");
    fs::create_dir_all(&cat).expect("mkdir");
    let file = cat.join("small-functions.md");
    fs::write(
        &file,
        r#"---
kind: standard
name: small-functions
category: rust-style
importance: high
applies_to: ["**/*.rs"]
applies_to_languages: ["rust"]
applies_to_domains: ["style"]
---
Keep functions concise and cohesive.
"#,
    )
    .expect("write");

    let loaded = load_runtime_standards(&root).expect("load");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].name.as_str(), "small-functions");
    assert_eq!(loaded[0].category.as_str(), "rust-style");
}

#[test]
fn invalid_frontmatter_fails_closed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("standards");
    let cat = root.join("rust-style");
    fs::create_dir_all(&cat).expect("mkdir");
    let file = cat.join("broken.md");
    fs::write(
        &file,
        r#"---
kind: standard
name: not-broken
category: wrong-category
importance: impossible
---
text
"#,
    )
    .expect("write");

    let err = load_runtime_standards(&root).expect_err("must reject invalid metadata");
    assert!(
        err.to_string().contains("mismatch") || err.to_string().contains("invalid importance"),
        "unexpected error: {err}"
    );
}
