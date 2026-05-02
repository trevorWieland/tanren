//! BDD tag and coverage validator (`xtask check-bdd-tags`).
//!
//! Enforces F-0002's locked BDD convention. The contract is documented in
//! `docs/architecture/subsystems/behavior-proof.md` (BDD Tagging And File
//! Convention) and reflected in `tests/bdd/README.md`. Summary:
//!
//! - One `.feature` per behavior, named `B-XXXX-<slug>.feature`.
//! - Feature-level `@B-XXXX` tag matches the filename. No other tags at
//!   feature level.
//! - Each `Scenario` carries exactly one of `@positive` / `@falsification`
//!   and 1–2 interface tags drawn from `@web @api @mcp @cli @tui`.
//! - Two-interface scenarios require a preceding `# rationale: …` comment.
//! - `Scenario Outline` and `Examples` are forbidden.
//! - The behavior file under `docs/behaviors/` must exist and be
//!   `product_status: accepted`.
//! - Strict-equality coverage: the union of interface tags across the
//!   feature's scenarios equals the behavior's frontmatter `interfaces:`.
//!   For each interface, ≥1 `@positive` scenario is required; when the
//!   DAG node's `expected_evidence.witnesses` includes `falsification`,
//!   ≥1 `@falsification` scenario per interface is also required.

mod data;
mod parser;

use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use data::{BehaviorRecord, EvidenceRecord, load_behaviors, load_dag_evidence};
use parser::{ParsedFeature, ParsedScenario, parse_feature, parse_filename};

const INTERFACE_TAGS: &[&str] = &["@web", "@api", "@mcp", "@cli", "@tui"];
const WITNESS_TAGS: &[&str] = &["@positive", "@falsification"];

pub(crate) fn run(workspace_root: &Path) -> Result<()> {
    let features_dir = workspace_root.join("tests").join("bdd").join("features");
    let behaviors_dir = workspace_root.join("docs").join("behaviors");
    let dag_path = workspace_root.join("docs").join("roadmap").join("dag.json");

    let feature_files = collect_feature_files(&features_dir);
    if feature_files.is_empty() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-bdd-tags: 0 feature files under tests/bdd/features (BDD convention upheld vacuously)"
        );
        return Ok(());
    }

    let behaviors = load_behaviors(&behaviors_dir)?;
    let dag_evidence = load_dag_evidence(&dag_path)?;

    let mut violations: Vec<String> = Vec::new();
    for path in &feature_files {
        validate_feature_file(
            path,
            workspace_root,
            &behaviors,
            &dag_evidence,
            &mut violations,
        );
    }

    if violations.is_empty() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-bdd-tags: {} feature file(s) validated; convention upheld",
            feature_files.len()
        );
        return Ok(());
    }

    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    for v in &violations {
        let _ = writeln!(handle, "{v}");
    }
    bail!(
        "check-bdd-tags: {} violation(s); see docs/architecture/subsystems/behavior-proof.md (BDD Tagging And File Convention)",
        violations.len()
    );
}

fn collect_feature_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !dir.exists() {
        return out;
    }
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("feature"))
        {
            out.push(path.to_path_buf());
        }
    }
    out.sort();
    out
}

fn validate_feature_file(
    path: &Path,
    workspace_root: &Path,
    behaviors: &HashMap<String, BehaviorRecord>,
    dag_evidence: &HashMap<String, EvidenceRecord>,
    violations: &mut Vec<String>,
) {
    let rel = path.strip_prefix(workspace_root).unwrap_or(path);

    let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
        violations.push(format!("{}: invalid filename", rel.display()));
        return;
    };
    let Some((expected_id, _slug)) = parse_filename(file_name) else {
        violations.push(format!(
            "{}: filename must match `B-XXXX-<slug>.feature`",
            rel.display()
        ));
        return;
    };

    let content = match fs::read_to_string(path).with_context(|| format!("read {}", path.display()))
    {
        Ok(c) => c,
        Err(err) => {
            violations.push(format!("{}: read failed ({err:#})", rel.display()));
            return;
        }
    };
    let parsed = parse_feature(&content);

    check_forbidden_keywords(rel, &parsed, violations);
    check_feature_tags(rel, &parsed, &expected_id, violations);
    let behavior = check_behavior_catalog(rel, &expected_id, behaviors, violations);

    let coverage = check_scenarios(rel, &parsed, violations);

    if let Some(b) = behavior {
        check_coverage_against_behavior(rel, &expected_id, b, &coverage, violations);
        check_coverage_against_dag(
            rel,
            &expected_id,
            &b.interfaces,
            &coverage,
            dag_evidence,
            violations,
        );
    }
}

#[derive(Default)]
struct ScenarioCoverage {
    positive_by_iface: BTreeMap<String, usize>,
    falsification_by_iface: BTreeMap<String, usize>,
}

fn check_forbidden_keywords(rel: &Path, parsed: &ParsedFeature, violations: &mut Vec<String>) {
    for line in &parsed.scenario_outline_lines {
        violations.push(format!(
            "{}:{line}: `Scenario Outline` is forbidden by F-0002 BDD convention",
            rel.display()
        ));
    }
    for line in &parsed.examples_lines {
        violations.push(format!(
            "{}:{line}: `Examples:` blocks are forbidden by F-0002 BDD convention",
            rel.display()
        ));
    }
}

fn check_feature_tags(
    rel: &Path,
    parsed: &ParsedFeature,
    expected_id: &str,
    violations: &mut Vec<String>,
) {
    let line = parsed.feature_tag_line.unwrap_or(0);
    if parsed.feature_tags.len() != 1 || parsed.feature_tags[0] != format!("@{expected_id}") {
        violations.push(format!(
            "{}:{line}: feature-level tags must be exactly [@{expected_id}], got {:?}",
            rel.display(),
            parsed.feature_tags
        ));
    }
}

fn check_behavior_catalog<'a>(
    rel: &Path,
    expected_id: &str,
    behaviors: &'a HashMap<String, BehaviorRecord>,
    violations: &mut Vec<String>,
) -> Option<&'a BehaviorRecord> {
    match behaviors.get(expected_id) {
        Some(b) if b.product_status == "accepted" => Some(b),
        Some(b) => {
            violations.push(format!(
                "{}: behavior {expected_id} has product_status={:?}; only `accepted` behaviors may have feature files",
                rel.display(),
                b.product_status
            ));
            Some(b)
        }
        None => {
            violations.push(format!(
                "{}: behavior {expected_id} has no docs/behaviors/{expected_id}-*.md catalog entry",
                rel.display()
            ));
            None
        }
    }
}

fn check_scenarios(
    rel: &Path,
    parsed: &ParsedFeature,
    violations: &mut Vec<String>,
) -> ScenarioCoverage {
    let mut coverage = ScenarioCoverage::default();
    let allowed = allowed_tag_set();
    for scenario in &parsed.scenarios {
        check_scenario(rel, scenario, &allowed, &mut coverage, violations);
    }
    coverage
}

fn check_scenario(
    rel: &Path,
    scenario: &ParsedScenario,
    allowed: &HashSet<&'static str>,
    coverage: &mut ScenarioCoverage,
    violations: &mut Vec<String>,
) {
    let line = scenario.keyword_line;
    let mut witness_count = 0;
    let mut interface_tags: Vec<String> = Vec::new();
    let mut is_positive = false;
    let mut is_falsification = false;
    for tag in &scenario.tags {
        if !allowed.contains(tag.as_str()) {
            violations.push(format!(
                "{}:{line}: scenario tag {tag} is not in the closed allowlist (\
                @positive, @falsification, @web, @api, @mcp, @cli, @tui)",
                rel.display()
            ));
            continue;
        }
        if tag == "@positive" {
            witness_count += 1;
            is_positive = true;
        } else if tag == "@falsification" {
            witness_count += 1;
            is_falsification = true;
        } else if INTERFACE_TAGS.contains(&tag.as_str()) {
            interface_tags.push(tag.clone());
        }
    }
    if witness_count != 1 {
        violations.push(format!(
            "{}:{line}: scenario must carry exactly one of @positive/@falsification (got {witness_count})",
            rel.display()
        ));
    }
    if interface_tags.is_empty() {
        violations.push(format!(
            "{}:{line}: scenario must carry at least one interface tag (@web/@api/@mcp/@cli/@tui)",
            rel.display()
        ));
    }
    if interface_tags.len() > 2 {
        violations.push(format!(
            "{}:{line}: scenario carries {} interface tags; max 2 allowed",
            rel.display(),
            interface_tags.len()
        ));
    }
    if interface_tags.len() == 2 && scenario.rationale.is_none() {
        violations.push(format!(
            "{}:{line}: 2-interface scenario requires a preceding `# rationale: <one line>` comment",
            rel.display()
        ));
    }

    if witness_count == 1 {
        for tag in &interface_tags {
            let iface = tag.trim_start_matches('@').to_owned();
            if is_positive {
                *coverage.positive_by_iface.entry(iface.clone()).or_insert(0) += 1;
            }
            if is_falsification {
                *coverage.falsification_by_iface.entry(iface).or_insert(0) += 1;
            }
        }
    }
}

fn check_coverage_against_behavior(
    rel: &Path,
    expected_id: &str,
    b: &BehaviorRecord,
    coverage: &ScenarioCoverage,
    violations: &mut Vec<String>,
) {
    let declared = &b.interfaces;
    let scenario_iface_union: BTreeSet<String> = coverage
        .positive_by_iface
        .keys()
        .chain(coverage.falsification_by_iface.keys())
        .cloned()
        .collect();
    for iface in scenario_iface_union.difference(declared) {
        violations.push(format!(
            "{}: interface tag @{iface} is not in behavior {expected_id} frontmatter `interfaces:` {:?}",
            rel.display(),
            declared
        ));
    }
    for iface in declared {
        if coverage.positive_by_iface.get(iface).copied().unwrap_or(0) == 0 {
            violations.push(format!(
                "{}: behavior {expected_id} declares interface @{iface} but no @positive scenario covers it",
                rel.display()
            ));
        }
    }
}

fn check_coverage_against_dag(
    rel: &Path,
    expected_id: &str,
    declared: &BTreeSet<String>,
    coverage: &ScenarioCoverage,
    dag_evidence: &HashMap<String, EvidenceRecord>,
    violations: &mut Vec<String>,
) {
    let Some(ev) = dag_evidence.get(expected_id) else {
        violations.push(format!(
            "{}: behavior {expected_id} has a feature file but no DAG node declares evidence for it",
            rel.display()
        ));
        return;
    };
    if ev.witnesses.contains("falsification") {
        for iface in declared {
            if coverage
                .falsification_by_iface
                .get(iface)
                .copied()
                .unwrap_or(0)
                == 0
            {
                violations.push(format!(
                    "{}: DAG node {} for behavior {expected_id} lists falsification witnesses; interface @{iface} has no @falsification scenario",
                    rel.display(),
                    ev.node_id
                ));
            }
        }
    }
    if &ev.interfaces != declared {
        violations.push(format!(
            "{}: DAG evidence interfaces {:?} disagree with behavior frontmatter {:?}",
            rel.display(),
            ev.interfaces,
            declared
        ));
    }
}

fn allowed_tag_set() -> HashSet<&'static str> {
    let mut set: HashSet<&'static str> = HashSet::new();
    for w in WITNESS_TAGS {
        set.insert(w);
    }
    for i in INTERFACE_TAGS {
        set.insert(i);
    }
    set
}
