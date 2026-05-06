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
//!   and 1–2 surface tags drawn from the project surface registry.
//! - Two-surface scenarios require a preceding `# rationale: …` comment.
//! - `Scenario Outline` and `Examples` are forbidden.
//! - The behavior file under `docs/behaviors/` must exist and be
//!   `product_status: accepted`.
//! - Strict-equality coverage: the union of surface tags across the
//!   feature's scenarios equals the behavior's frontmatter `surfaces:`
//!   declaration, with `interfaces:` accepted as a migration alias.
//!   For each surface, ≥1 `@positive` scenario is required; when the
//!   DAG node's `expected_evidence.witnesses` includes `falsification`,
//!   ≥1 `@falsification` scenario per surface is also required.

mod data;
mod parser;
mod surfaces;

use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use data::{BehaviorRecord, EvidenceRecord, load_behaviors, load_dag_evidence};
use parser::{ParsedFeature, ParsedScenario, parse_feature, parse_filename};
use surfaces::SurfaceRegistry;

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
    let surface_registry = SurfaceRegistry::load(workspace_root)?;

    let mut violations: Vec<String> = Vec::new();
    // Track which behavior IDs already have a feature file so the
    // convention's "one .feature per behavior" rule fails fast across
    // files (each file otherwise validates in isolation, so two files
    // like `B-0123-a.feature` and `B-0123-b.feature` would each pass
    // their own checks and silently violate the global invariant).
    let mut behavior_owners: HashMap<String, PathBuf> = HashMap::new();
    for path in &feature_files {
        validate_feature_file(
            path,
            workspace_root,
            &behaviors,
            &dag_evidence,
            &surface_registry,
            &mut behavior_owners,
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
    surface_registry: &SurfaceRegistry,
    behavior_owners: &mut HashMap<String, PathBuf>,
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

    if let Some(prior) = behavior_owners.get(&expected_id) {
        let prior_rel = prior.strip_prefix(workspace_root).unwrap_or(prior);
        violations.push(format!(
            "{}: behavior {expected_id} is already proven by {} — F-0002 BDD convention requires exactly one .feature per behavior",
            rel.display(),
            prior_rel.display()
        ));
    } else {
        behavior_owners.insert(expected_id.clone(), path.to_path_buf());
    }

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

    let coverage = check_scenarios(rel, &parsed, surface_registry, violations);

    if let Some(b) = behavior {
        check_coverage_against_behavior(
            rel,
            &expected_id,
            b,
            &coverage,
            surface_registry,
            violations,
        );
        check_coverage_against_dag(
            rel,
            &expected_id,
            &b.surfaces,
            &coverage,
            dag_evidence,
            violations,
        );
    }
}

#[derive(Default)]
struct ScenarioCoverage {
    positive_by_surface: BTreeMap<String, usize>,
    falsification_by_surface: BTreeMap<String, usize>,
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
    for line in &parsed.stray_tag_lines {
        violations.push(format!(
            "{}:{line}: tag block precedes `Background:` / `Rule:`; tags belong at the feature header or immediately above a `Scenario:`",
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
    surface_registry: &SurfaceRegistry,
    violations: &mut Vec<String>,
) -> ScenarioCoverage {
    let mut coverage = ScenarioCoverage::default();
    for scenario in &parsed.scenarios {
        check_scenario(rel, scenario, surface_registry, &mut coverage, violations);
    }
    coverage
}

fn check_scenario(
    rel: &Path,
    scenario: &ParsedScenario,
    surface_registry: &SurfaceRegistry,
    coverage: &mut ScenarioCoverage,
    violations: &mut Vec<String>,
) {
    let line = scenario.keyword_line;
    let mut witness_count = 0;
    let mut surface_tags: Vec<String> = Vec::new();
    let mut is_positive = false;
    let mut is_falsification = false;
    for tag in &scenario.tags {
        if tag == "@positive" {
            witness_count += 1;
            is_positive = true;
        } else if tag == "@falsification" {
            witness_count += 1;
            is_falsification = true;
        } else if let Some(surface_id) = tag.strip_prefix('@') {
            if surface_registry.contains(surface_id) {
                surface_tags.push(tag.clone());
            } else {
                violations.push(format!(
                    "{}:{line}: scenario tag {tag} is not in the closed allowlist (\
                    @positive, @falsification, {})",
                    rel.display(),
                    surface_registry.tag_display()
                ));
            }
        } else {
            violations.push(format!(
                "{}:{line}: scenario tag {tag} is not in the closed allowlist (\
                @positive, @falsification, {})",
                rel.display(),
                surface_registry.tag_display()
            ));
        }
    }
    if witness_count != 1 {
        violations.push(format!(
            "{}:{line}: scenario must carry exactly one of @positive/@falsification (got {witness_count})",
            rel.display()
        ));
    }
    if surface_tags.is_empty() {
        violations.push(format!(
            "{}:{line}: scenario must carry at least one surface tag ({})",
            rel.display(),
            surface_registry.tag_display()
        ));
    }
    if surface_tags.len() > 2 {
        violations.push(format!(
            "{}:{line}: scenario carries {} surface tags; max 2 allowed",
            rel.display(),
            surface_tags.len()
        ));
    }
    // Reject both missing rationale and empty `# rationale:` (the parser
    // captures `# rationale:` with no body as `Some("")` — without this
    // empty-string guard a bare `# rationale:` would silently satisfy
    // the convention that calls for a one-line justification).
    if surface_tags.len() == 2
        && scenario
            .rationale
            .as_deref()
            .is_none_or(|s| s.trim().is_empty())
    {
        violations.push(format!(
            "{}:{line}: 2-surface scenario requires a non-empty preceding `# rationale: <one line>` comment",
            rel.display()
        ));
    }

    if witness_count == 1 {
        for tag in &surface_tags {
            let surface = tag.trim_start_matches('@').to_owned();
            if is_positive {
                *coverage
                    .positive_by_surface
                    .entry(surface.clone())
                    .or_insert(0) += 1;
            }
            if is_falsification {
                *coverage
                    .falsification_by_surface
                    .entry(surface)
                    .or_insert(0) += 1;
            }
        }
    }
}

fn check_coverage_against_behavior(
    rel: &Path,
    expected_id: &str,
    b: &BehaviorRecord,
    coverage: &ScenarioCoverage,
    surface_registry: &SurfaceRegistry,
    violations: &mut Vec<String>,
) {
    let declared = &b.surfaces;
    for surface in declared {
        if !surface_registry.contains(surface) {
            violations.push(format!(
                "{}: behavior {expected_id} declares unknown surface {surface:?}; docs/experience/surfaces.yml allows [{}]",
                rel.display(),
                surface_registry.tag_display()
            ));
        }
    }
    let scenario_surface_union: BTreeSet<String> = coverage
        .positive_by_surface
        .keys()
        .chain(coverage.falsification_by_surface.keys())
        .cloned()
        .collect();
    for surface in scenario_surface_union.difference(declared) {
        violations.push(format!(
            "{}: surface tag @{surface} is not in behavior {expected_id} frontmatter `surfaces:`/`interfaces:` {:?}",
            rel.display(),
            declared
        ));
    }
    for surface in declared {
        if coverage
            .positive_by_surface
            .get(surface)
            .copied()
            .unwrap_or(0)
            == 0
        {
            violations.push(format!(
                "{}: behavior {expected_id} declares surface @{surface} but no @positive scenario covers it",
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
        for surface in declared {
            if coverage
                .falsification_by_surface
                .get(surface)
                .copied()
                .unwrap_or(0)
                == 0
            {
                violations.push(format!(
                    "{}: DAG node {} for behavior {expected_id} lists falsification witnesses; surface @{surface} has no @falsification scenario",
                    rel.display(),
                    ev.node_id
                ));
            }
        }
    }
    if &ev.surfaces != declared {
        violations.push(format!(
            "{}: DAG evidence surfaces {:?} disagree with behavior frontmatter {:?}",
            rel.display(),
            ev.surfaces,
            declared
        ));
    }
}
