//! Public-surface renderer tests. Moved out of `src/methodology/renderer.rs`
//! to keep that file under the 500-line budget after the Lane 0.5
//! diagnostic-span additions.

use std::collections::{BTreeMap, HashMap};

use tanren_app_services::methodology::renderer::{CanonicalBytes, render_command};
use tanren_app_services::methodology::source::{CommandFamily, CommandFrontmatter, CommandSource};

fn source_with(vars: Vec<String>, body: &str) -> CommandSource {
    CommandSource {
        name: "do-task".into(),
        family: CommandFamily::SpecLoop,
        frontmatter: CommandFrontmatter {
            name: "do-task".into(),
            role: "implementation".into(),
            orchestration_loop: true,
            autonomy: "autonomous".into(),
            declared_variables: vars,
            declared_tools: vec![],
            required_capabilities: vec![],
            produces_evidence: vec![],
            description: None,
            agent: None,
            model: None,
            subtask: None,
            extensions: Default::default(),
        },
        body: body.into(),
        source_path: "x".into(),
    }
}

#[test]
fn happy_path_renders() {
    let src = source_with(vec!["HOOK".into()], "run {{HOOK}} now");
    let mut ctx = HashMap::new();
    ctx.insert("HOOK".into(), "just check".into());
    let r = render_command(&src, &ctx).expect("ok");
    assert_eq!(r.body, "run just check now");
}

#[test]
fn declared_unused_errors() {
    let src = source_with(vec!["UNUSED".into()], "no vars here");
    let ctx = HashMap::new();
    assert!(render_command(&src, &ctx).is_err());
}

#[test]
fn undeclared_reference_errors() {
    let src = source_with(vec![], "call {{MISSING}}");
    let ctx = HashMap::new();
    assert!(render_command(&src, &ctx).is_err());
}

#[test]
fn unresolved_reference_errors() {
    let src = source_with(vec!["NEED".into()], "{{NEED}}");
    let ctx = HashMap::new();
    assert!(render_command(&src, &ctx).is_err());
}

#[test]
fn canonicalize_trims_and_ends_with_lf() {
    let b = CanonicalBytes::canonicalize("line  \r\nnext  \n\n\n");
    let s = String::from_utf8(b.0).expect("utf8");
    assert_eq!(s, "line\nnext\n");
}

#[test]
fn fingerprint_is_stable() {
    let a = CanonicalBytes::canonicalize("body");
    let b = CanonicalBytes::canonicalize("body");
    assert_eq!(a.fingerprint(), b.fingerprint());
}

#[test]
fn utf8_preserved_around_substitution() {
    let body = "Record signposts — feed future audits … with {{HOOK}} → é中🔥";
    let src = source_with(vec!["HOOK".into()], body);
    let mut ctx = HashMap::new();
    ctx.insert("HOOK".into(), "just check".into());
    let r = render_command(&src, &ctx).expect("ok");
    assert_eq!(
        r.body,
        "Record signposts — feed future audits … with just check → é中🔥"
    );
}

#[test]
fn frontmatter_vars_are_extracted_and_substituted() {
    let src = CommandSource {
        name: "demo".into(),
        family: CommandFamily::SpecLoop,
        frontmatter: CommandFrontmatter {
            name: "demo".into(),
            role: "impl".into(),
            orchestration_loop: false,
            autonomy: "autonomous".into(),
            declared_variables: vec!["PRODUCT_ROOT".into()],
            declared_tools: vec![],
            required_capabilities: vec![],
            produces_evidence: vec!["{{PRODUCT_ROOT}}/spec.md".into()],
            description: None,
            agent: None,
            model: None,
            subtask: None,
            extensions: Default::default(),
        },
        body: "see {{PRODUCT_ROOT}}".into(),
        source_path: "x".into(),
    };
    let mut ctx = HashMap::new();
    ctx.insert("PRODUCT_ROOT".into(), "docs".into());
    let r = render_command(&src, &ctx).expect("ok");
    assert_eq!(r.frontmatter.produces_evidence, vec!["docs/spec.md"]);
    assert_eq!(r.body, "see docs");
}

#[test]
fn extension_namespace_vars_are_extracted_and_substituted() {
    let mut extensions = BTreeMap::new();
    extensions.insert(
        "custom_hint".into(),
        serde_yaml::Value::String("Run {{PRODUCT_ROOT}} safely".into()),
    );
    let src = CommandSource {
        name: "demo".into(),
        family: CommandFamily::SpecLoop,
        frontmatter: CommandFrontmatter {
            name: "demo".into(),
            role: "impl".into(),
            orchestration_loop: false,
            autonomy: "autonomous".into(),
            declared_variables: vec!["PRODUCT_ROOT".into()],
            declared_tools: vec![],
            required_capabilities: vec![],
            produces_evidence: vec![],
            description: None,
            agent: None,
            model: None,
            subtask: None,
            extensions,
        },
        body: "see {{PRODUCT_ROOT}}".into(),
        source_path: "x".into(),
    };
    let mut ctx = HashMap::new();
    ctx.insert("PRODUCT_ROOT".into(), "docs".into());
    let r = render_command(&src, &ctx).expect("ok");
    assert_eq!(r.body, "see docs");
    assert_eq!(
        r.frontmatter.extensions.get("custom_hint"),
        Some(&serde_yaml::Value::String("Run docs safely".into()))
    );
}
