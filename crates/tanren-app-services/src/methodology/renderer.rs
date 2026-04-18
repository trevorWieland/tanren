//! Template variable renderer.
//!
//! Per Lane 0.5 non-negotiable #5, unknown / declared-but-unused /
//! referenced-but-undeclared template variables are **hard errors**.
//! This renderer enforces those rules:
//!
//! 1. Every `{{VAR}}` token in the body must have a matching entry in
//!    `declared_variables` on the command's frontmatter.
//! 2. Every entry in `declared_variables` must be referenced at least
//!    once in the body.
//! 3. Every referenced variable must have a value in the resolution
//!    context or the render fails.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::errors::{MethodologyError, MethodologyResult};
use super::source::CommandSource;

/// Rendered command ready for format-specific wrapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedCommand {
    pub name: String,
    pub family: super::source::CommandFamily,
    pub frontmatter: super::source::CommandFrontmatter,
    pub body: String,
}

/// Canonical byte representation (LF line endings, stripped trailing
/// whitespace, single final newline). Used for cross-target parity
/// hashing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalBytes(pub Vec<u8>);

impl CanonicalBytes {
    /// Canonicalize a rendered body.
    #[must_use]
    pub fn canonicalize(body: &str) -> Self {
        let mut s = body.replace("\r\n", "\n");
        // Strip trailing whitespace on each line.
        s = s.lines().map(str::trim_end).collect::<Vec<_>>().join("\n");
        // Ensure a single final newline, no BOM.
        while s.ends_with("\n\n") {
            s.pop();
        }
        if !s.ends_with('\n') {
            s.push('\n');
        }
        Self(s.into_bytes())
    }

    /// Sha-like fingerprint for tests + parity assertions.
    ///
    /// Uses [`std::hash::DefaultHasher`] — not cryptographically
    /// strong, but sufficient for drift detection and parity assertion.
    #[must_use]
    pub fn fingerprint(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.0.hash(&mut h);
        h.finish()
    }
}

/// Render a single source command against a variable context.
///
/// # Errors
/// See [`MethodologyError`].
pub fn render_command(
    source: &CommandSource,
    context: &HashMap<String, String>,
) -> MethodologyResult<RenderedCommand> {
    let declared: BTreeSet<&str> = source
        .frontmatter
        .declared_variables
        .iter()
        .map(String::as_str)
        .collect();
    // References appear in both body and frontmatter string values.
    let mut referenced = extract_references(&source.body);
    let fm_refs_owned = frontmatter_references(&source.frontmatter);
    for r in &fm_refs_owned {
        referenced.insert(r.as_str());
    }

    // Rule 2: declared-but-unused = hard error.
    let unused: Vec<&&str> = declared
        .iter()
        .filter(|v| !referenced.contains(**v))
        .collect();
    if !unused.is_empty() {
        return Err(MethodologyError::Validation(format!(
            "command `{}`: declared but unused variables: {unused:?}",
            source.name
        )));
    }

    // Rule 1: referenced-but-undeclared = hard error.
    let undeclared: Vec<&str> = referenced
        .iter()
        .filter(|v| !declared.contains(*v))
        .copied()
        .collect();
    if !undeclared.is_empty() {
        return Err(MethodologyError::Validation(format!(
            "command `{}`: referenced but undeclared variables: {undeclared:?}",
            source.name
        )));
    }

    // Rule 3: every referenced variable must resolve.
    let unresolved: Vec<&str> = referenced
        .iter()
        .filter(|v| !context.contains_key(**v))
        .copied()
        .collect();
    if !unresolved.is_empty() {
        return Err(MethodologyError::Validation(format!(
            "command `{}`: variables referenced but not supplied: {unresolved:?}",
            source.name
        )));
    }

    let body = substitute(&source.body, context);
    let frontmatter = substitute_frontmatter(&source.frontmatter, context);

    Ok(RenderedCommand {
        name: source.name.clone(),
        family: source.family,
        frontmatter,
        body,
    })
}

/// Collect every `{{VAR}}` reference from the string fields of a
/// frontmatter block (declared_tools, required_capabilities,
/// produces_evidence, extras, name, role, autonomy).
fn frontmatter_references(fm: &super::source::CommandFrontmatter) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    let mut visit = |s: &str| {
        for r in extract_references(s) {
            out.insert(r.to_owned());
        }
    };
    visit(&fm.name);
    visit(&fm.role);
    visit(&fm.autonomy);
    for v in &fm.declared_tools {
        visit(v);
    }
    for v in &fm.required_capabilities {
        visit(v);
    }
    for v in &fm.produces_evidence {
        visit(v);
    }
    for v in fm.extras.values() {
        collect_yaml_refs(v, &mut out);
    }
    out.into_iter().collect()
}

fn collect_yaml_refs(v: &serde_yaml::Value, out: &mut BTreeSet<String>) {
    match v {
        serde_yaml::Value::String(s) => {
            for r in extract_references(s) {
                out.insert(r.to_owned());
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            for child in seq {
                collect_yaml_refs(child, out);
            }
        }
        serde_yaml::Value::Mapping(map) => {
            for child in map.values() {
                collect_yaml_refs(child, out);
            }
        }
        _ => {}
    }
}

fn substitute_frontmatter(
    fm: &super::source::CommandFrontmatter,
    ctx: &HashMap<String, String>,
) -> super::source::CommandFrontmatter {
    let mut out = fm.clone();
    out.name = substitute(&out.name, ctx);
    out.role = substitute(&out.role, ctx);
    out.autonomy = substitute(&out.autonomy, ctx);
    out.declared_tools = out
        .declared_tools
        .iter()
        .map(|s| substitute(s, ctx))
        .collect();
    out.required_capabilities = out
        .required_capabilities
        .iter()
        .map(|s| substitute(s, ctx))
        .collect();
    out.produces_evidence = out
        .produces_evidence
        .iter()
        .map(|s| substitute(s, ctx))
        .collect();
    let mut new_extras = BTreeMap::new();
    for (k, v) in &out.extras {
        new_extras.insert(k.clone(), substitute_yaml(v, ctx));
    }
    out.extras = new_extras;
    out
}

fn substitute_yaml(v: &serde_yaml::Value, ctx: &HashMap<String, String>) -> serde_yaml::Value {
    match v {
        serde_yaml::Value::String(s) => serde_yaml::Value::String(substitute(s, ctx)),
        serde_yaml::Value::Sequence(seq) => {
            serde_yaml::Value::Sequence(seq.iter().map(|c| substitute_yaml(c, ctx)).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let mut out = serde_yaml::Mapping::new();
            for (k, val) in map {
                out.insert(k.clone(), substitute_yaml(val, ctx));
            }
            serde_yaml::Value::Mapping(out)
        }
        other => other.clone(),
    }
}

/// Extract the set of unique `{{VAR}}` tokens from a body.
///
/// UTF-8-safe: walks the string by char boundaries via `find`, never
/// treats bytes as characters.
fn extract_references(body: &str) -> BTreeSet<&str> {
    let mut out = BTreeSet::new();
    let mut rest = body;
    while let Some(open) = rest.find("{{") {
        let after = &rest[open + 2..];
        if let Some(close) = after.find("}}") {
            let name = after[..close].trim();
            if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                // Re-derive the slice inside `body` with original
                // lifetime so callers get `&str` references tied to
                // `body` rather than a copy.
                let abs_start = body.len() - rest.len() + open + 2;
                let abs_end = abs_start + close;
                let trimmed = body[abs_start..abs_end].trim();
                out.insert(trimmed);
            }
            rest = &after[close + 2..];
        } else {
            break;
        }
    }
    out
}

/// Replace every `{{VAR}}` with its resolved value. Assumes all
/// references resolve — callers must have validated first via
/// [`render_command`].
///
/// UTF-8-safe: operates on string slices and `push_str` only; never
/// casts `u8 as char`.
fn substitute(body: &str, ctx: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        if let Some(close) = after.find("}}") {
            let name = after[..close].trim();
            if let Some(value) = ctx.get(name) {
                out.push_str(value);
                rest = &after[close + 2..];
                continue;
            }
            // Unresolved token: preserve the literal `{{name}}` so
            // downstream audits can flag it. `render_command` rejects
            // unresolved references before this path, but if called
            // without validation (e.g. by a future debug helper) we
            // still produce valid UTF-8.
            out.push_str("{{");
            out.push_str(&after[..close]);
            out.push_str("}}");
            rest = &after[close + 2..];
        } else {
            // No closing `}}` — copy the rest verbatim and stop.
            out.push_str("{{");
            out.push_str(after);
            return out;
        }
    }
    out.push_str(rest);
    out
}

/// Render every command in a catalog. Returns the rendered set plus a
/// map of every variable referenced across the catalog so the caller
/// can detect a config declaring variables none of the commands use.
///
/// # Errors
/// See [`render_command`].
pub fn render_catalog(
    catalog: &[CommandSource],
    context: &HashMap<String, String>,
) -> MethodologyResult<(Vec<RenderedCommand>, BTreeMap<String, Vec<String>>)> {
    let mut rendered = Vec::with_capacity(catalog.len());
    let mut references: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for c in catalog {
        for v in extract_references(&c.body) {
            references
                .entry(v.to_owned())
                .or_default()
                .push(c.name.clone());
        }
        for v in frontmatter_references(&c.frontmatter) {
            references.entry(v).or_default().push(c.name.clone());
        }
        rendered.push(render_command(c, context)?);
    }
    Ok((rendered, references))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::methodology::source::{CommandFamily, CommandFrontmatter};

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
                extras: Default::default(),
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
        // Multi-byte chars: em-dash (—, 3 bytes), ellipsis (…, 3 bytes),
        // arrow (→, 3 bytes), Latin accented (é, 2 bytes), CJK (中, 3
        // bytes), emoji (🔥, 4 bytes). A byte-wise `as char` cast would
        // corrupt every one.
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
    fn extract_references_is_utf8_safe() {
        let refs = extract_references("— {{FOO}} … {{BAR}} →");
        let set: Vec<&str> = refs.iter().copied().collect();
        assert_eq!(set, vec!["BAR", "FOO"]);
    }

    #[test]
    fn substitute_over_multibyte_gaps() {
        let mut ctx = HashMap::new();
        ctx.insert("A".into(), "x".into());
        ctx.insert("B".into(), "y".into());
        assert_eq!(substitute("—{{A}}—{{B}}—", &ctx), "—x—y—");
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
                extras: Default::default(),
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
}
