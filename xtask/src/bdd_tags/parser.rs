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
    /// Lines where tags were observed immediately before a structural
    /// keyword that does not consume tags (`Background:`, `Rule:`).
    /// The convention forbids tag-block placement other than feature-
    /// or scenario-level, so these are reported as violations rather
    /// than silently floated forward to the next `Scenario:`.
    pub stray_tag_lines: Vec<usize>,
}

#[derive(Debug)]
pub(super) struct ParsedScenario {
    pub keyword_line: usize,
    pub tags: Vec<String>,
    pub rationale: Option<String>,
    pub step_lines: Vec<String>,
}

/// Parse a `.feature` file by line. Tag groups float forward to attach to
/// the next `Feature:` or `Scenario:` keyword. `# rationale: …` comments
/// are captured as the next scenario's rationale.
pub(super) fn parse_feature(content: &str) -> ParsedFeature {
    let mut state = ScanState::default();
    for (idx, raw_line) in content.lines().enumerate() {
        let lineno = idx + 1;
        parse_feature_line(raw_line, lineno, &mut state);
    }
    state.finish()
}

#[derive(Debug, Default)]
struct ScanState {
    feature_tags: Vec<String>,
    feature_tag_line: Option<usize>,
    scenarios: Vec<ParsedScenario>,
    scenario_outline_lines: Vec<usize>,
    examples_lines: Vec<usize>,
    stray_tag_lines: Vec<usize>,
    pending_tags: Vec<String>,
    pending_tag_line: Option<usize>,
    pending_rationale: Option<String>,
    current_scenario_idx: Option<usize>,
}

impl ScanState {
    fn finish(self) -> ParsedFeature {
        ParsedFeature {
            feature_tags: self.feature_tags,
            feature_tag_line: self.feature_tag_line,
            scenarios: self.scenarios,
            scenario_outline_lines: self.scenario_outline_lines,
            examples_lines: self.examples_lines,
            stray_tag_lines: self.stray_tag_lines,
        }
    }
}

fn parse_feature_line(raw_line: &str, lineno: usize, state: &mut ScanState) {
    let trimmed = raw_line.trim_start();

    if trimmed.is_empty() {
        break_rationale_adjacency(state);
        return;
    }
    if handle_rationale_comment(trimmed, state) {
        return;
    }
    if handle_tag_line(trimmed, lineno, state) {
        return;
    }
    if handle_structural_keyword(trimmed, lineno, state) {
        return;
    }
    if handle_step_capture(trimmed, state) {
        return;
    }
    // Steps, doc strings, tables, and any other unrecognised line
    // also break rationale adjacency. Pending tags are left alone
    // here because Gherkin allows stray content only inside step
    // blocks (which we never reach with active pending_tags), but
    // a rationale must not float over arbitrary content.
    break_rationale_adjacency(state);
}

fn handle_rationale_comment(trimmed: &str, state: &mut ScanState) -> bool {
    let Some(rest) = trimmed.strip_prefix('#') else {
        return false;
    };
    let rest = rest.trim();
    if let Some(value) = rest.strip_prefix("rationale:") {
        state.pending_rationale = Some(value.trim().to_owned());
    } else {
        // Non-rationale comments also break adjacency. Only contiguous
        // `# rationale:` + tag lines may carry a rationale forward to
        // the next `Scenario:`.
        break_rationale_adjacency(state);
    }
    true
}

fn handle_tag_line(trimmed: &str, lineno: usize, state: &mut ScanState) -> bool {
    if !trimmed.starts_with('@') {
        return false;
    }
    state.current_scenario_idx = None;
    for token in trimmed.split_whitespace() {
        if token.starts_with('@') {
            state.pending_tags.push(token.to_owned());
        }
    }
    if state.pending_tag_line.is_none() {
        state.pending_tag_line = Some(lineno);
    }
    true
}

fn handle_structural_keyword(trimmed: &str, lineno: usize, state: &mut ScanState) -> bool {
    if trimmed.starts_with("Feature:") {
        state.current_scenario_idx = None;
        state.feature_tags = std::mem::take(&mut state.pending_tags);
        state.feature_tag_line = state.pending_tag_line.take();
        break_rationale_adjacency(state);
        return true;
    }
    if trimmed.starts_with("Scenario Outline:") {
        state.current_scenario_idx = None;
        state.scenario_outline_lines.push(lineno);
        clear_pending_tags(state);
        break_rationale_adjacency(state);
        return true;
    }
    if trimmed.starts_with("Examples:") {
        state.examples_lines.push(lineno);
        return true;
    }
    if trimmed.starts_with("Scenario:") {
        state.scenarios.push(ParsedScenario {
            keyword_line: lineno,
            tags: std::mem::take(&mut state.pending_tags),
            rationale: state.pending_rationale.take(),
            step_lines: Vec::new(),
        });
        state.current_scenario_idx = Some(state.scenarios.len() - 1);
        state.pending_tag_line = None;
        return true;
    }
    if trimmed.starts_with("Background:") || trimmed.starts_with("Rule:") {
        state.current_scenario_idx = None;
        // Tag/rationale must not float past structural keywords other
        // than `Scenario:`. Capture the tag-block start line as a
        // stray-tag violation rather than silently attaching tags to a
        // later scenario.
        if !state.pending_tags.is_empty()
            && let Some(line) = state.pending_tag_line
        {
            state.stray_tag_lines.push(line);
        }
        clear_pending_tags(state);
        break_rationale_adjacency(state);
        return true;
    }
    false
}

fn handle_step_capture(trimmed: &str, state: &mut ScanState) -> bool {
    let Some(step_text) = parse_step_text(trimmed) else {
        return false;
    };
    let Some(idx) = state.current_scenario_idx else {
        return false;
    };
    state.scenarios[idx].step_lines.push(step_text.to_owned());
    true
}

fn clear_pending_tags(state: &mut ScanState) {
    state.pending_tags.clear();
    state.pending_tag_line = None;
}

fn break_rationale_adjacency(state: &mut ScanState) {
    // The convention requires `# rationale:` to sit immediately above
    // the tag block so rationale does not float to unrelated scenarios.
    state.pending_rationale = None;
}

fn parse_step_text(line: &str) -> Option<&str> {
    for keyword in ["Given ", "When ", "Then ", "And ", "But ", "* "] {
        if let Some(step) = line.strip_prefix(keyword) {
            return Some(step.trim());
        }
    }
    None
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
