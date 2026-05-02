//! Hand-rolled Gherkin tag scanner for `xtask check-bdd-tags`.
//!
//! The cucumber crate validates structure at test-run time; this scanner
//! only walks `.feature` files for tags, scenario keywords, and filename
//! shape so the validator can run in seconds without pulling a parser
//! crate.

#[derive(Debug)]
pub(super) struct ParsedFeature {
    pub feature_tags: Vec<String>,
    pub feature_tag_line: Option<usize>,
    pub scenarios: Vec<ParsedScenario>,
    pub scenario_outline_lines: Vec<usize>,
    pub examples_lines: Vec<usize>,
}

#[derive(Debug)]
pub(super) struct ParsedScenario {
    pub keyword_line: usize,
    pub tags: Vec<String>,
    pub rationale: Option<String>,
}

/// Parse a `.feature` file by line. Tag groups float forward to attach to
/// the next `Feature:` or `Scenario:` keyword. `# rationale: …` comments
/// are captured as the next scenario's rationale.
pub(super) fn parse_feature(content: &str) -> ParsedFeature {
    let mut feature_tags: Vec<String> = Vec::new();
    let mut feature_tag_line: Option<usize> = None;
    let mut scenarios: Vec<ParsedScenario> = Vec::new();
    let mut scenario_outline_lines: Vec<usize> = Vec::new();
    let mut examples_lines: Vec<usize> = Vec::new();

    let mut pending_tags: Vec<String> = Vec::new();
    let mut pending_tag_line: Option<usize> = None;
    let mut pending_rationale: Option<String> = None;

    for (idx, raw_line) in content.lines().enumerate() {
        let lineno = idx + 1;
        let trimmed = raw_line.trim_start();

        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('#') {
            let rest = rest.trim();
            if let Some(value) = rest.strip_prefix("rationale:") {
                pending_rationale = Some(value.trim().to_owned());
            }
            continue;
        }
        if trimmed.starts_with('@') {
            for token in trimmed.split_whitespace() {
                if token.starts_with('@') {
                    pending_tags.push(token.to_owned());
                }
            }
            if pending_tag_line.is_none() {
                pending_tag_line = Some(lineno);
            }
            continue;
        }
        if trimmed.starts_with("Feature:") {
            feature_tags = std::mem::take(&mut pending_tags);
            feature_tag_line = pending_tag_line.take();
            pending_rationale = None;
            continue;
        }
        if trimmed.starts_with("Scenario Outline:") {
            scenario_outline_lines.push(lineno);
            pending_tags.clear();
            pending_tag_line = None;
            pending_rationale = None;
            continue;
        }
        if trimmed.starts_with("Examples:") {
            examples_lines.push(lineno);
            continue;
        }
        if trimmed.starts_with("Scenario:") {
            scenarios.push(ParsedScenario {
                keyword_line: lineno,
                tags: std::mem::take(&mut pending_tags),
                rationale: pending_rationale.take(),
            });
            pending_tag_line = None;
            continue;
        }
        if trimmed.starts_with("Background:") || trimmed.starts_with("Rule:") {
            // Tag/rationale must not float past structural keywords other
            // than `Scenario:`.
            pending_rationale = None;
        }
        // Steps, doc strings, and tables are passthrough — nothing to do.
    }

    ParsedFeature {
        feature_tags,
        feature_tag_line,
        scenarios,
        scenario_outline_lines,
        examples_lines,
    }
}

/// Parse `B-XXXX-<slug>.feature` filenames. Returns the behavior ID and the
/// slug if the shape matches; `None` otherwise.
pub(super) fn parse_filename(name: &str) -> Option<(String, String)> {
    let stem = name.strip_suffix(".feature")?;
    let mut parts = stem.splitn(3, '-');
    let prefix = parts.next()?;
    let digits = parts.next()?;
    let slug = parts.next()?;
    if prefix != "B" || digits.len() != 4 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if slug.is_empty() {
        return None;
    }
    Some((format!("B-{digits}"), slug.to_owned()))
}
