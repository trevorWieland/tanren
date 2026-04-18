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
            "[TANREN_RENDER_DECLARED_UNUSED] command `{}` ({}): declared but unused variables: {unused:?}. Remove them from `declared_variables` or reference them in the body/frontmatter.",
            source.name,
            source.source_path.display(),
        )));
    }

    // Rule 1: referenced-but-undeclared = hard error.
    let undeclared: Vec<&str> = referenced
        .iter()
        .filter(|v| !declared.contains(*v))
        .copied()
        .collect();
    if !undeclared.is_empty() {
        let locs = variable_locations(&source.body, &undeclared);
        return Err(MethodologyError::Validation(format!(
            "[TANREN_RENDER_UNDECLARED_VAR] command `{}` ({}): referenced but undeclared variables: {undeclared:?}. First-occurrence locations: {locs}. Add them to `declared_variables` in the command frontmatter.",
            source.name,
            source.source_path.display(),
            locs = format_locations(&locs, source.source_path.to_string_lossy().as_ref()),
        )));
    }

    // Rule 3: every referenced variable must resolve.
    let unresolved: Vec<&str> = referenced
        .iter()
        .filter(|v| !context.contains_key(**v))
        .copied()
        .collect();
    if !unresolved.is_empty() {
        let locs = variable_locations(&source.body, &unresolved);
        return Err(MethodologyError::Validation(format!(
            "[TANREN_RENDER_UNKNOWN_VAR] command `{}` ({}): variables referenced but not supplied: {unresolved:?}. First-occurrence locations: {locs}. Supply values in the context (env var, tanren.yml `variables:`, or CLI flag).",
            source.name,
            source.source_path.display(),
            locs = format_locations(&locs, source.source_path.to_string_lossy().as_ref()),
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

/// Record of where a `{{VAR}}` first appears in the source body.
/// Used by diagnostic messages so the `[TANREN_RENDER_*]` error
/// codes include concrete `file:line:col` remediation hints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableLocation {
    pub variable: String,
    pub line: u32,
    pub col: u32,
}

fn variable_locations(body: &str, vars: &[&str]) -> Vec<VariableLocation> {
    let mut out: Vec<VariableLocation> = Vec::with_capacity(vars.len());
    let mut line_no = 1_u32;
    let mut col_no = 1_u32;
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            // Find matching `}}`.
            let mut j = i + 2;
            while j + 1 < bytes.len() && !(bytes[j] == b'}' && bytes[j + 1] == b'}') {
                j += 1;
            }
            if j + 1 < bytes.len() {
                // `body[i+2..j]` is the variable name (possibly whitespace-padded).
                let name = body[i + 2..j].trim();
                if vars.contains(&name) && !out.iter().any(|l| l.variable == name) {
                    out.push(VariableLocation {
                        variable: name.to_owned(),
                        line: line_no,
                        col: col_no,
                    });
                }
            }
        }
        if bytes[i] == b'\n' {
            line_no = line_no.saturating_add(1);
            col_no = 1;
        } else {
            col_no = col_no.saturating_add(1);
        }
        i += 1;
    }
    out
}

fn format_locations(locs: &[VariableLocation], file: &str) -> String {
    if locs.is_empty() {
        return "<frontmatter or non-body reference>".into();
    }
    let mut parts = Vec::with_capacity(locs.len());
    for l in locs {
        parts.push(format!("{}:{}:{} ({})", file, l.line, l.col, l.variable));
    }
    parts.join(", ")
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

    // Tests that exercise the private helpers `extract_references` and
    // `substitute`. Public-surface renderer tests live in
    // `crates/tanren-app-services/tests/methodology_renderer.rs` to
    // keep this file under the 500-line budget.

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
}
