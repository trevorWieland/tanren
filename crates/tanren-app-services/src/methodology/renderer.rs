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
    let referenced = extract_references(&source.body);

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

    Ok(RenderedCommand {
        name: source.name.clone(),
        family: source.family,
        frontmatter: source.frontmatter.clone(),
        body,
    })
}

/// Extract the set of unique `{{VAR}}` tokens from a body.
fn extract_references(body: &str) -> BTreeSet<&str> {
    let mut out = BTreeSet::new();
    let bytes = body.as_bytes();
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if let Some(rel_end) = body[i + 2..].find("}}") {
                let start = i + 2;
                let end = start + rel_end;
                let name = body[start..end].trim();
                if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    out.insert(
                        &body[start..end]
                            .trim_start_matches(' ')
                            .trim_end_matches(' ')[..],
                    );
                }
                i = end + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Replace every `{{VAR}}` with its resolved value. Assumes all
/// references resolve — callers must have validated first via
/// [`render_command`].
fn substitute(body: &str, ctx: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(body.len());
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 3 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if let Some(rel_end) = body[i + 2..].find("}}") {
                let start = i + 2;
                let end = start + rel_end;
                let name = body[start..end].trim();
                if let Some(value) = ctx.get(name) {
                    out.push_str(value);
                    i = end + 2;
                    continue;
                }
            }
        }
        out.push(body.as_bytes()[i] as char);
        i += 1;
    }
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
}
