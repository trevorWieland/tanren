//! `xtask check-web-harness-routes` — enforce canonical ownership for web
//! harness routes and generated projection drift.
//!
//! The `apps/web/src/app/harness/[behaviorId]/**` tree is behavior-proof
//! infrastructure, not product routing. Each harness route must be owned by a
//! canonical behavior ID that has `@web` witness coverage in
//! `tests/bdd/features/**/*.feature`, and the checked-in TypeScript projection
//! consumed by the web app must stay in sync with that canonical inventory.

use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::Path;

const FEATURES_DIR_REL: &str = "tests/bdd/features";
const HARNESS_APP_DIR_REL: &str = "apps/web/src/app/harness/[behaviorId]";
const GENERATED_ROUTES_REL: &str = "apps/web/src/lib/generated/behavior-harness-routes.ts";

#[derive(Debug, Clone, Copy)]
struct HarnessOwnership {
    leaf: &'static str,
    behavior_id: &'static str,
}

// Source of truth for harness route leaves. This table is validated against the
// canonical feature/interface inventory before projection generation.
const HARNESS_ROUTE_OWNERSHIP: &[HarnessOwnership] = &[HarnessOwnership {
    leaf: "organizations",
    behavior_id: "B-0066",
}];

#[derive(Debug, Clone)]
struct BehaviorFeatureInventory {
    behavior_id: String,
    feature_path: String,
    has_web_interface: bool,
}

#[derive(Debug, Clone)]
struct GeneratedHarnessRoute {
    leaf: String,
    behavior_id: String,
    behavior_segment: String,
    feature_path: String,
    route: String,
}

pub(crate) fn run(root: &Path) -> Result<()> {
    let inventory = load_feature_inventory(root)?;
    let harness_pages = collect_harness_page_leaves(root)?;

    let mut violations: Vec<String> = Vec::new();
    let mut seen_leaves: BTreeSet<&str> = BTreeSet::new();
    let mut generated_routes: Vec<GeneratedHarnessRoute> = Vec::new();

    for ownership in HARNESS_ROUTE_OWNERSHIP {
        if !seen_leaves.insert(ownership.leaf) {
            violations.push(format!(
                "{GENERATED_ROUTES_REL}: duplicate harness ownership declaration for leaf `{}`",
                ownership.leaf
            ));
            continue;
        }

        let Some(feature) = inventory.get(ownership.behavior_id) else {
            violations.push(format!(
                "{GENERATED_ROUTES_REL}: behavior {} has no canonical feature under {FEATURES_DIR_REL}",
                ownership.behavior_id
            ));
            continue;
        };

        if !feature.has_web_interface {
            violations.push(format!(
                "{GENERATED_ROUTES_REL}: behavior {} lacks @web witness coverage in {}",
                ownership.behavior_id, feature.feature_path
            ));
            continue;
        }

        let behavior_segment = feature.behavior_id.to_ascii_lowercase();
        let route = format!("/harness/{behavior_segment}/{}", ownership.leaf);
        generated_routes.push(GeneratedHarnessRoute {
            leaf: ownership.leaf.to_owned(),
            behavior_id: feature.behavior_id.clone(),
            behavior_segment,
            feature_path: feature.feature_path.clone(),
            route,
        });

        if !harness_pages.contains(ownership.leaf) {
            violations.push(format!(
                "{HARNESS_APP_DIR_REL}/{}: missing page.tsx for declared harness ownership of behavior {}",
                ownership.leaf, ownership.behavior_id
            ));
        }
    }

    for leaf in &harness_pages {
        if !seen_leaves.contains(leaf.as_str()) {
            violations.push(format!(
                "{HARNESS_APP_DIR_REL}/{leaf}/page.tsx exists but no canonical harness ownership is declared"
            ));
        }
    }

    generated_routes.sort_by(|a, b| a.leaf.cmp(&b.leaf));
    let expected_projection = render_projection(&generated_routes);
    let generated_path = root.join(GENERATED_ROUTES_REL);
    match fs::read_to_string(&generated_path) {
        Ok(actual) => {
            if actual != expected_projection {
                violations.push(format!(
                    "{} is out of sync with canonical feature/interface ownership; regenerate by running `cargo run -q -p tanren-xtask -- check-web-harness-routes --root {}` and applying the emitted projection",
                    generated_path.display(),
                    root.display()
                ));
            }
        }
        Err(err) => {
            violations.push(format!(
                "{}: unable to read generated harness projection ({err})",
                generated_path.display()
            ));
        }
    }

    if violations.is_empty() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-web-harness-routes: 0 violations (harness ownership and generated projection are in sync)"
        );
        return Ok(());
    }

    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    for violation in &violations {
        let _ = writeln!(handle, "{violation}");
    }
    bail!(
        "check-web-harness-routes: {} violation(s); keep web harness routes owned by canonical behavior proof inventory",
        violations.len()
    )
}

fn load_feature_inventory(root: &Path) -> Result<BTreeMap<String, BehaviorFeatureInventory>> {
    let features_dir = root.join(FEATURES_DIR_REL);
    let mut inventory = BTreeMap::<String, BehaviorFeatureInventory>::new();
    if !features_dir.exists() {
        return Ok(inventory);
    }

    for entry in walkdir::WalkDir::new(&features_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "feature") {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(behavior_id) = parse_behavior_id_from_feature_filename(file_name) else {
            continue;
        };

        let content = fs::read_to_string(path)
            .with_context(|| format!("read feature file {}", path.display()))?;
        let has_web_interface = feature_declares_web_interface(&content);
        let rel_path = path.strip_prefix(root).unwrap_or(path);
        let normalized_path = rel_path.to_string_lossy().replace('\\', "/");

        inventory.insert(
            behavior_id.clone(),
            BehaviorFeatureInventory {
                behavior_id,
                feature_path: normalized_path,
                has_web_interface,
            },
        );
    }

    Ok(inventory)
}

fn parse_behavior_id_from_feature_filename(file_name: &str) -> Option<String> {
    let stem = file_name.strip_suffix(".feature")?;
    let mut parts = stem.splitn(3, '-');
    let prefix = parts.next()?;
    let digits = parts.next()?;
    let _slug = parts.next()?;
    if prefix != "B" || digits.len() != 4 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("B-{digits}"))
}

fn feature_declares_web_interface(content: &str) -> bool {
    for raw in content.lines() {
        let line = raw.trim_start();
        if !line.starts_with('@') {
            continue;
        }
        if line.split_whitespace().any(|token| token == "@web") {
            return true;
        }
    }
    false
}

fn collect_harness_page_leaves(root: &Path) -> Result<BTreeSet<String>> {
    let harness_dir = root.join(HARNESS_APP_DIR_REL);
    let mut leaves = BTreeSet::new();
    if !harness_dir.exists() {
        return Ok(leaves);
    }

    for entry in
        fs::read_dir(&harness_dir).with_context(|| format!("read_dir {}", harness_dir.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let leaf = entry.file_name().to_string_lossy().to_string();
        let page = entry.path().join("page.tsx");
        if page.exists() {
            leaves.insert(leaf);
        }
    }

    Ok(leaves)
}

fn render_projection(routes: &[GeneratedHarnessRoute]) -> String {
    let mut output = String::new();
    output.push_str(
        "// This file is generated by `cargo run -q -p tanren-xtask -- check-web-harness-routes`.\n",
    );
    output.push_str("// Do not edit manually.\n\n");
    output.push_str("export const BEHAVIOR_HARNESS_ROUTES = {\n");

    for route in routes {
        let _ = write!(
            output,
            "  {}: {{\n    behaviorId: \"{}\",\n    behaviorSegment: \"{}\",\n    featurePath: \"{}\",\n    route: \"{}\",\n  }},\n",
            route.leaf, route.behavior_id, route.behavior_segment, route.feature_path, route.route
        );
    }

    output.push_str(
        "} as const satisfies Record<\n  string,\n  {\n    behaviorId: `B-${string}`;\n    behaviorSegment: `b-${string}`;\n    featurePath: string;\n    route: `/harness/${string}`;\n  }\n>;\n\n",
    );
    output.push_str(
        "export type BehaviorHarnessRouteLeaf = keyof typeof BEHAVIOR_HARNESS_ROUTES;\n\n",
    );
    output.push_str(
        "export function lookupBehaviorHarnessRoute(\n  leaf: string,\n  behaviorSegment: string,\n): (typeof BEHAVIOR_HARNESS_ROUTES)[BehaviorHarnessRouteLeaf] | null {\n  const candidate = BEHAVIOR_HARNESS_ROUTES[leaf as BehaviorHarnessRouteLeaf];\n  if (!candidate) {\n    return null;\n  }\n  return candidate.behaviorSegment === behaviorSegment ? candidate : null;\n}\n",
    );

    output
}
