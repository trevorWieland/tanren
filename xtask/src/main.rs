use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use chrono::{SecondsFormat, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::Serialize;
use serde_json::{Map, Value, json};
use uuid::Uuid;

const BDD_CRATE: &str = "crates/tanren-bdd";
const BEHAVIOR_DOCS_DIR: &str = "docs/behaviors";
const BDD_FEATURES_DIR: &str = "tests/bdd/features";
const BEHAVIOR_OUTPUT_ROOT: &str = "artifacts/behavior/enforced";
const BDD_OUTPUT_ROOT: &str = "artifacts/bdd/enforced";
const COVERAGE_OUTPUT_ROOT: &str = "artifacts/coverage/enforced";
const MUTATION_OUTPUT_ROOT: &str = "artifacts/mutation/enforced";
const BEHAVIOR_SOURCE_A: &str = "crates/tanren-bdd/src/steps/installer.rs";
const BEHAVIOR_SOURCE_B: &str = "crates/tanren-bdd/src/world.rs";
const BEHAVIOR_SOURCE_C: &str = "crates/tanren-testkit/src/process.rs";

const CURRENT_POLICY_FILES: &[&str] = &[
    "justfile",
    "lefthook.yml",
    ".github/workflows/rust-ci.yml",
    "docs/roadmap/README.md",
    "README.md",
    "CLAUDE.md",
];
const FORBIDDEN_POLICY_SNIPPETS: &[&str] = &[
    "tanren-bdd-phase0",
    "tests/bdd/phase0",
    "docs/roadmap/proof/phase",
    "scripts/proof/phase0",
    "BEH-P0-",
    "check-phase0-",
    "check-phase0-gates",
    "check-phase0-stage-gates",
    "phase0-gates",
    "bench-redaction",
    "check-redaction-perf",
    "cargo nextest run --workspace",
    "run_postgres_integration.sh",
    "integration-postgres",
    "postgres-integration",
    "test-hooks",
];
const SUMMARY_OUTCOME_FILES: &[(&str, &str)] = &[
    ("caught", "caught.txt"),
    ("missed", "missed.txt"),
    ("timeout", "timeout.txt"),
    ("unviable", "unviable.txt"),
];
const SURVIVOR_OUTCOME_FILES: &[(&str, &str)] =
    &[("missed", "missed.txt"), ("timeout", "timeout.txt")];
type WaveBehaviorIds = BTreeMap<String, Vec<String>>;

#[derive(Debug, Clone)]
struct BehaviorDoc {
    id: String,
    title: String,
    status: String,
    path: String,
}

#[derive(Debug, Clone)]
struct FeatureScenario {
    behavior_id: String,
    witness: String,
    title: String,
    feature_file: String,
    line: usize,
    tags: Vec<String>,
}

#[derive(Debug, Clone)]
struct BehaviorInventory {
    docs: BTreeMap<String, BehaviorDoc>,
    scenarios: Vec<FeatureScenario>,
    errors: Vec<String>,
}

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        usage();
        bail!("missing xtask command");
    };
    let rest = args.collect::<Vec<_>>();
    match command.as_str() {
        "behavior" => behavior_command(&rest),
        "check-rust-test-surface" => check_rust_test_surface(),
        "validate-behavior-traceability" => behavior_validate(),
        "mint-actor-token" => mint_actor_token(&rest),
        "mint-mcp-capability-envelope" => mint_mcp_capability_envelope(&rest),
        "render-mutation-triage" => render_mutation_triage(&rest),
        "render-coverage-classification" => render_coverage_classification(&rest),
        _ => {
            usage();
            bail!("unknown xtask command: {command}")
        }
    }
}

fn usage() {
    eprintln!(
        "usage: cargo run -p tanren-xtask -- <behavior|check-rust-test-surface|validate-behavior-traceability|mint-actor-token|mint-mcp-capability-envelope|render-mutation-triage|render-coverage-classification> [args]"
    );
}

#[derive(Debug, Default)]
struct Args {
    values: BTreeMap<String, String>,
}

impl Args {
    fn parse(raw: &[String]) -> Result<Self> {
        let mut values = BTreeMap::new();
        let mut iter = raw.iter();
        while let Some(key) = iter.next() {
            if !key.starts_with("--") {
                bail!("expected --key argument, got {key}");
            }
            let Some(value) = iter.next() else {
                bail!("missing value for {key}");
            };
            values.insert(key.trim_start_matches("--").to_string(), value.to_string());
        }
        Ok(Self { values })
    }

    fn required_path(&self, key: &str) -> Result<PathBuf> {
        self.values
            .get(key)
            .map(PathBuf::from)
            .with_context(|| format!("missing --{key}"))
    }

    fn required_string(&self, key: &str) -> Result<String> {
        self.values
            .get(key)
            .cloned()
            .with_context(|| format!("missing --{key}"))
    }

    fn required_i32(&self, key: &str) -> Result<i32> {
        self.required_string(key)?
            .parse::<i32>()
            .with_context(|| format!("invalid integer for --{key}"))
    }

    fn string_or_empty(&self, key: &str) -> String {
        self.values.get(key).cloned().unwrap_or_default()
    }
}

#[derive(Debug, Default)]
struct FlagArgs {
    values: BTreeMap<String, String>,
    flags: BTreeSet<String>,
}

impl FlagArgs {
    fn parse(raw: &[String], allowed_flags: &[&str]) -> Result<Self> {
        let allowed = allowed_flags.iter().copied().collect::<BTreeSet<_>>();
        let mut values = BTreeMap::new();
        let mut flags = BTreeSet::new();
        let mut iter = raw.iter().peekable();
        while let Some(key) = iter.next() {
            if !key.starts_with("--") {
                bail!("expected --key argument, got {key}");
            }
            let normalized = key.trim_start_matches("--");
            if allowed.contains(normalized) {
                flags.insert(normalized.to_string());
                continue;
            }
            let Some(value) = iter.next() else {
                bail!("missing value for {key}");
            };
            if value.starts_with("--") {
                bail!("missing value for {key}");
            }
            values.insert(normalized.to_string(), value.to_string());
        }
        Ok(Self { values, flags })
    }

    fn required_string(&self, key: &str) -> Result<String> {
        self.values
            .get(key)
            .cloned()
            .with_context(|| format!("missing --{key}"))
    }

    fn string_or(&self, key: &str, default: &str) -> String {
        self.values
            .get(key)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }

    fn optional_string(&self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }

    fn required_path(&self, key: &str) -> Result<PathBuf> {
        self.values
            .get(key)
            .map(PathBuf::from)
            .with_context(|| format!("missing --{key}"))
    }

    fn i64_or(&self, key: &str, default: i64) -> Result<i64> {
        self.values.get(key).map_or(Ok(default), |value| {
            value
                .parse::<i64>()
                .with_context(|| format!("invalid integer for --{key}"))
        })
    }

    fn has_flag(&self, key: &str) -> bool {
        self.flags.contains(key)
    }
}

#[derive(Debug, Serialize)]
struct ActorClaims {
    iss: String,
    aud: String,
    exp: i64,
    nbf: i64,
    iat: i64,
    jti: String,
    org_id: String,
    user_id: String,
}

#[derive(Debug, Serialize)]
struct CapabilityClaims {
    iss: String,
    aud: String,
    exp: i64,
    nbf: i64,
    iat: i64,
    jti: String,
    phase: String,
    spec_id: String,
    agent_session_id: String,
    capabilities: Vec<String>,
}

fn mint_actor_token(raw: &[String]) -> Result<()> {
    let args = FlagArgs::parse(raw, &["token-only"])?;
    let private_key_path = args.required_path("private-key-pem")?;
    let issuer = args.required_string("issuer")?;
    let audience = args.required_string("audience")?;
    let org_id = args.required_string("org-id")?;
    let user_id = args.required_string("user-id")?;
    let mode = args.required_string("mode")?;
    let requested_ttl = args.i64_or("requested-ttl", 600)?;
    let max_ttl = args.i64_or("max-ttl", 900)?;
    let iat = args.i64_or("iat", Utc::now().timestamp())?;
    let kid = args.string_or("kid", "tanren-proof");
    let mut iss = issuer.clone();
    let mut aud = audience.clone();
    let mut jti = args
        .optional_string("jti")
        .unwrap_or_else(|| Uuid::now_v7().to_string());

    match mode.as_str() {
        "valid" | "expired" | "ttl_over_max" => {}
        "wrong_issuer" => iss = format!("{issuer}-wrong"),
        "wrong_audience" => aud = format!("{audience}-wrong"),
        "replay_reuse" => {
            jti = args
                .optional_string("jti")
                .unwrap_or_else(|| "tanren-proof-replay-jti".to_string());
        }
        _ => bail!("unsupported --mode {mode}"),
    }

    let (exp, nbf) = if mode == "expired" {
        (iat - 1, iat - 300)
    } else if mode == "ttl_over_max" {
        (iat + requested_ttl.max(max_ttl + 1), iat - 30)
    } else {
        (iat + requested_ttl, iat - 30)
    };
    let exp_minus_iat = exp - iat;
    eprintln!("iat={iat} exp={exp} exp_minus_iat={exp_minus_iat} max_ttl={max_ttl}");

    let ttl_violation = exp_minus_iat > max_ttl;
    if ttl_violation {
        eprintln!("warning: exp_minus_iat exceeds actor_token_max_ttl_secs");
    }
    if ttl_violation && mode != "ttl_over_max" {
        bail!("refusing to mint token because exp_minus_iat exceeds max_ttl");
    }

    let claims = ActorClaims {
        iss,
        aud,
        exp,
        nbf,
        iat,
        jti,
        org_id,
        user_id,
    };
    let token = sign_eddsa_jwt(&private_key_path, &kid, &claims)?;
    if args.has_flag("token-only") {
        println!("{token}");
    } else {
        let payload = json!({
            "mode": mode,
            "issuer": claims.iss,
            "audience": claims.aud,
            "iat": claims.iat,
            "exp": claims.exp,
            "exp_minus_iat": exp_minus_iat,
            "max_ttl": max_ttl,
            "ttl_violation": ttl_violation,
            "jti": claims.jti,
            "token": token,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    }
    Ok(())
}

fn mint_mcp_capability_envelope(raw: &[String]) -> Result<()> {
    let args = FlagArgs::parse(raw, &["diagnostics-stderr", "token-only"])?;
    let private_key_path = args.required_path("private-key-pem")?;
    let issuer = args.required_string("issuer")?;
    let audience = args.required_string("audience")?;
    let phase = args.required_string("phase")?;
    let spec_id = args.required_string("spec-id")?;
    let agent_session_id = args.required_string("agent-session-id")?;
    let capabilities = args
        .required_string("capabilities")?
        .split(',')
        .map(str::trim)
        .filter(|capability| !capability.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if capabilities.is_empty() {
        bail!("--capabilities must include at least one capability tag");
    }

    let requested_ttl = args.i64_or("requested-ttl", 600)?;
    let max_ttl = args.i64_or("max-ttl", 900)?;
    let iat = args.i64_or("iat", Utc::now().timestamp())?;
    let exp = iat + requested_ttl;
    let nbf = iat - 30;
    let ttl = exp - iat;
    if ttl <= 0 || ttl > max_ttl {
        bail!("requested ttl out of bounds (exp-iat={ttl}, max_ttl={max_ttl})");
    }
    if args.has_flag("diagnostics-stderr") {
        eprintln!("iat={iat} exp={exp} exp_minus_iat={ttl} max_ttl={max_ttl} phase={phase}");
    }

    let kid = args.string_or("kid", "tanren-mcp-capability");
    let claims = CapabilityClaims {
        iss: issuer,
        aud: audience,
        exp,
        nbf,
        iat,
        jti: args
            .optional_string("jti")
            .unwrap_or_else(|| Uuid::now_v7().to_string()),
        phase,
        spec_id,
        agent_session_id,
        capabilities,
    };
    let token = sign_eddsa_jwt(&private_key_path, &kid, &claims)?;
    if args.has_flag("token-only") {
        println!("{token}");
    } else {
        let payload = json!({
            "issuer": claims.iss,
            "audience": claims.aud,
            "phase": claims.phase,
            "spec_id": claims.spec_id,
            "agent_session_id": claims.agent_session_id,
            "capabilities": claims.capabilities,
            "iat": claims.iat,
            "exp": claims.exp,
            "exp_minus_iat": ttl,
            "max_ttl": max_ttl,
            "jti": claims.jti,
            "token": token,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    }
    Ok(())
}

fn sign_eddsa_jwt<T>(private_key_path: &Path, kid: &str, claims: &T) -> Result<String>
where
    T: Serialize,
{
    let key_bytes = fs::read(private_key_path)
        .with_context(|| format!("read private key {}", private_key_path.display()))?;
    let encoding_key = EncodingKey::from_ed_pem(&key_bytes)
        .with_context(|| format!("parse Ed25519 private key {}", private_key_path.display()))?;
    let mut header = Header::new(Algorithm::EdDSA);
    header.kid = Some(kid.to_string());
    encode(&header, claims, &encoding_key).context("sign JWT")
}

fn behavior_command(raw: &[String]) -> Result<()> {
    let Some((command, rest)) = raw.split_first() else {
        bail!(
            "usage: cargo run -p tanren-xtask -- behavior <inventory|validate|run|coverage|mutation|verify>"
        );
    };
    match command.as_str() {
        "inventory" => {
            let inventory = behavior_inventory()?;
            let run_dir = timestamped_dir(BEHAVIOR_OUTPUT_ROOT)?;
            write_json(
                &run_dir.join("inventory.json"),
                &behavior_inventory_json(&inventory),
            )?;
            update_latest(Path::new(BEHAVIOR_OUTPUT_ROOT), &run_dir)?;
            if !inventory.errors.is_empty() {
                print_behavior_errors(&inventory.errors);
                bail!("behavior inventory validation failed");
            }
            println!(
                "behavior: inventory={} artifact={}",
                inventory.docs.len(),
                run_dir.display()
            );
            Ok(())
        }
        "validate" => behavior_validate(),
        "run" => behavior_run(),
        "coverage" => behavior_coverage(),
        "mutation" => behavior_mutation(rest),
        "verify" => {
            behavior_validate()?;
            behavior_run()?;
            behavior_coverage()
        }
        other => bail!("unknown behavior command: {other}"),
    }
}

fn behavior_validate() -> Result<()> {
    let inventory = behavior_inventory()?;
    let run_dir = timestamped_dir(BEHAVIOR_OUTPUT_ROOT)?;
    write_json(
        &run_dir.join("inventory.json"),
        &behavior_inventory_json(&inventory),
    )?;
    update_latest(Path::new(BEHAVIOR_OUTPUT_ROOT), &run_dir)?;
    if !inventory.errors.is_empty() {
        print_behavior_errors(&inventory.errors);
        bail!("behavior validation failed");
    }
    println!(
        "behavior: accepted={} scenarios={} artifact={}",
        accepted_behavior_count(&inventory),
        inventory.scenarios.len(),
        run_dir.display()
    );
    Ok(())
}

fn behavior_run() -> Result<()> {
    let inventory = behavior_inventory()?;
    if !inventory.errors.is_empty() {
        print_behavior_errors(&inventory.errors);
        bail!("behavior validation failed");
    }

    run_command(
        Command::new("cargo").args(["build", "--locked", "-p", "tanren-cli", "-p", "tanren-bdd"]),
        "build behavior binaries",
    )?;

    let run_dir = timestamped_dir(BDD_OUTPUT_ROOT)?;
    let stdout = run_dir.join("bdd.stdout.log");
    let stderr = run_dir.join("bdd.stderr.log");
    let mut command = Command::new(target_debug_binary("tanren-bdd"));
    command
        .env("TANREN_BDD_FEATURE_PATH", BDD_FEATURES_DIR)
        .env(
            "TANREN_TEST_BIN_TANREN_CLI",
            target_debug_binary("tanren-cli"),
        );
    let status = run_command_capture(&mut command, &stdout, &stderr)?;
    let passed = status.success();
    write_json(
        &run_dir.join("run.json"),
        &json!({
            "schema_version": "1.0.0",
            "artifact": "bdd_run",
            "generated_at": now_timestamp(),
            "features_root": BDD_FEATURES_DIR,
            "status": if passed { "passed" } else { "failed" },
            "exit_code": status.code().unwrap_or(1),
            "scenario_count": inventory.scenarios.len(),
            "stdout": repo_path(&stdout),
            "stderr": repo_path(&stderr),
        }),
    )?;
    update_latest(Path::new(BDD_OUTPUT_ROOT), &run_dir)?;
    if !passed {
        bail!("behavior BDD run failed; see {}", run_dir.display());
    }
    println!(
        "bdd: scenarios={} artifact={}",
        inventory.scenarios.len(),
        run_dir.display()
    );
    Ok(())
}

fn behavior_coverage() -> Result<()> {
    let inventory = behavior_inventory()?;
    if !inventory.errors.is_empty() {
        print_behavior_errors(&inventory.errors);
        bail!("behavior validation failed");
    }
    let run_dir = timestamped_dir(COVERAGE_OUTPUT_ROOT)?;
    let coverage_summary = run_dir.join("coverage-summary.json");
    let stdout = run_dir.join("coverage-run.stdout.log");
    let stderr = run_dir.join("coverage-run.stderr.log");
    let report_stdout = run_dir.join("coverage-report.stdout.log");
    let report_stderr = run_dir.join("coverage-report.stderr.log");

    run_command(
        Command::new("cargo").args(["llvm-cov", "clean", "--workspace"]),
        "clean coverage data",
    )?;

    let mut coverage = Command::new("cargo");
    coverage
        .args([
            "llvm-cov",
            "run",
            "--no-report",
            "--package",
            "tanren-bdd",
            "--bin",
            "tanren-bdd",
            "--locked",
        ])
        .env("TANREN_BDD_FEATURE_PATH", BDD_FEATURES_DIR)
        .env("TANREN_BDD_BIN_MODE", "build-once");
    let status = run_command_capture(&mut coverage, &stdout, &stderr)?;

    let mut summary = Command::new("cargo");
    summary
        .args([
            "llvm-cov",
            "report",
            "--summary-only",
            "--json",
            "--output-path",
        ])
        .arg(&coverage_summary)
        .arg("--locked");
    let summary_status = run_command_capture(&mut summary, &report_stdout, &report_stderr)?;

    let mut lcov = Command::new("cargo");
    lcov.args(["llvm-cov", "report", "--lcov", "--output-path"])
        .arg(run_dir.join("lcov.info"))
        .arg("--locked");
    let lcov_status = run_command_capture(
        &mut lcov,
        &run_dir.join("coverage-lcov.stdout.log"),
        &run_dir.join("coverage-lcov.stderr.log"),
    )?;

    let coverage_payload = load_json_or_empty(&coverage_summary)?;
    let (coverage_by_file, llvm_totals) = load_coverage_summary(&coverage_payload)?;
    let source_lines_by_file = workspace_rust_sources(Path::new("."))?;
    let (workspace_by_file, workspace_totals) =
        build_workspace_coverage(&coverage_by_file, &source_lines_by_file);
    let uncovered_product_code = uncovered_product_code(&workspace_by_file);

    write_json(
        &run_dir.join("classification.json"),
        &json!({
            "schema_version": "1.0.0",
            "artifact": "behavior_coverage",
            "generated_at": now_timestamp(),
            "run": {
                "status": if status.success() && summary_status.success() && lcov_status.success() { "passed" } else { "failed" },
                "exit_code": status.code().unwrap_or(1),
                "summary_exit_code": summary_status.code().unwrap_or(1),
                "subprocess_strategy": "cargo-llvm-cov run with build-once tanren-cli subprocess",
            },
            "behavior_witnesses": behavior_witness_summary(&inventory),
            "coverage_summary": {
                "workspace_totals": workspace_totals,
                "llvm_measured_totals": llvm_totals,
                "uncovered_product_code": uncovered_product_code,
            },
            "artifacts": {
                "coverage_summary_json": repo_path(&coverage_summary),
                "lcov": repo_path(&run_dir.join("lcov.info")),
                "report_stdout": repo_path(&report_stdout),
                "report_stderr": repo_path(&report_stderr),
                "stdout": repo_path(&stdout),
                "stderr": repo_path(&stderr),
            }
        }),
    )?;
    update_latest(Path::new(COVERAGE_OUTPUT_ROOT), &run_dir)?;
    if !status.success() || !summary_status.success() || !lcov_status.success() {
        bail!("behavior coverage failed; see {}", run_dir.display());
    }
    println!("coverage: artifact={}", run_dir.display());
    Ok(())
}

fn behavior_mutation(raw: &[String]) -> Result<()> {
    let args = Args::parse(raw)?;
    let run_dir = timestamped_dir(MUTATION_OUTPUT_ROOT)?;
    let mutants_out = run_dir.join("mutants.out");
    let timeout = args
        .values
        .get("timeout")
        .cloned()
        .or_else(|| env::var("TANREN_MUTATION_TIMEOUT_SECS").ok())
        .unwrap_or_else(|| "300".to_string());
    let shard = args
        .values
        .get("shard")
        .cloned()
        .or_else(|| env::var("TANREN_MUTATION_SHARD").ok())
        .unwrap_or_else(|| "0/1".to_string());
    let package_shard = args
        .values
        .get("package-shard")
        .cloned()
        .or_else(|| env::var("TANREN_MUTATION_PACKAGE_SHARD").ok())
        .unwrap_or_else(|| "0/1".to_string());
    let selected_packages = mutation_product_packages(&package_shard)?;

    let mut command = Command::new("cargo");
    command
        .args([
            "mutants",
            "--workspace",
            "--test-package",
            "tanren-bdd",
            "--baseline=run",
            "--no-shuffle",
            "--timeout",
            &timeout,
            "--shard",
            &shard,
            "--exclude",
            "crates/tanren-bdd/**",
            "--exclude",
            "crates/tanren-testkit/**",
            "--exclude",
            "xtask/**",
            "--exclude",
            "artifacts/**",
            "--output",
        ])
        .arg(&mutants_out)
        .env("TANREN_BDD_FEATURE_PATH", absolute_path(BDD_FEATURES_DIR)?)
        .env("TANREN_BDD_BIN_MODE", "build-once");
    for package in &selected_packages {
        command.args(["--package", package]);
    }

    write_command_file(&run_dir.join("command.txt"), &command)?;
    let status = run_command_capture(
        &mut command,
        &run_dir.join("cargo-mutants.stdout.log"),
        &run_dir.join("cargo-mutants.stderr.log"),
    )?;
    let resolved_mutants_out = resolve_mutants_out_dir(&mutants_out);
    let counts = count_outcomes(&resolved_mutants_out)?;
    let missed = counts
        .get("missed_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let timeout_count = counts
        .get("timeout_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    write_json(
        &run_dir.join("triage.json"),
        &json!({
            "schema_version": "1.0.0",
            "artifact": "behavior_mutation",
            "generated_at": now_timestamp(),
            "run": {
                "status": if status.success() { "passed" } else { "failed" },
                "exit_code": status.code().unwrap_or(1),
                "shard": shard,
                "package_shard": package_shard,
                "packages": selected_packages,
                "timeout_secs": timeout,
                "test_package": "tanren-bdd",
                "scope": "workspace product crates",
                "subprocess_strategy": "build tanren-cli once per mutated workspace",
                "excluded": ["crates/tanren-bdd/**", "crates/tanren-testkit/**", "xtask/**", "artifacts/**"],
            },
            "outcomes": counts,
            "artifacts": {
                "mutants_out": repo_path(&resolved_mutants_out),
                "command": repo_path(&run_dir.join("command.txt")),
            }
        }),
    )?;
    update_latest(Path::new(MUTATION_OUTPUT_ROOT), &run_dir)?;
    if !status.success() || missed > 0 || timeout_count > 0 {
        bail!("behavior mutation failed; see {}", run_dir.display());
    }
    println!("mutation: artifact={}", run_dir.display());
    Ok(())
}

fn behavior_inventory() -> Result<BehaviorInventory> {
    let docs = load_behavior_docs(Path::new(BEHAVIOR_DOCS_DIR))?;
    let mut errors = validate_behavior_docs(&docs);
    let scenarios = load_feature_scenarios(Path::new(BDD_FEATURES_DIR), &docs, &mut errors)?;
    validate_witness_obligations(&docs, &scenarios, &mut errors);
    Ok(BehaviorInventory {
        docs,
        scenarios,
        errors,
    })
}

fn mutation_product_packages(shard: &str) -> Result<Vec<String>> {
    let (index, total) = parse_zero_based_shard(shard)?;
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .context("read cargo metadata for mutation package shard")?;
    if !output.status.success() {
        bail!(
            "cargo metadata failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let metadata: Value = serde_json::from_slice(&output.stdout).context("parse cargo metadata")?;
    let members: BTreeSet<String> = metadata
        .get("workspace_members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect();
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .context("cargo metadata missing packages")?;

    let mut product = Vec::new();
    for package in packages {
        let id = package
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !members.contains(id) {
            continue;
        }
        let name = package
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let manifest = package
            .get("manifest_path")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if matches!(name, "tanren-bdd" | "tanren-testkit" | "tanren-xtask")
            || manifest.contains("/xtask/")
        {
            continue;
        }
        product.push(name.to_owned());
    }
    product.sort();
    product.dedup();

    let selected = product
        .into_iter()
        .enumerate()
        .filter_map(|(offset, name)| (offset % total == index).then_some(name))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        bail!("mutation package shard {shard} selected no product packages");
    }
    Ok(selected)
}

fn parse_zero_based_shard(raw: &str) -> Result<(usize, usize)> {
    let Some((index, total)) = raw.split_once('/') else {
        bail!("invalid shard `{raw}`; expected N/M");
    };
    let index = index
        .parse::<usize>()
        .with_context(|| format!("parse shard index `{index}`"))?;
    let total = total
        .parse::<usize>()
        .with_context(|| format!("parse shard total `{total}`"))?;
    if total == 0 || index >= total {
        bail!("invalid shard `{raw}`; index must be lower than total");
    }
    Ok((index, total))
}

fn load_behavior_docs(root: &Path) -> Result<BTreeMap<String, BehaviorDoc>> {
    let mut docs = BTreeMap::new();
    let mut paths = Vec::new();
    collect_markdown_files(root, &mut paths)?;
    for path in paths {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with("B-") || !name.ends_with(".md") {
            continue;
        }
        let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let metadata = parse_frontmatter(&text)
            .with_context(|| format!("parse frontmatter {}", path.display()))?;
        let id = yaml_string(&metadata, "id").unwrap_or_default();
        let title = yaml_string(&metadata, "title").unwrap_or_default();
        let status = yaml_string(&metadata, "status").unwrap_or_default();
        let doc = BehaviorDoc {
            id: id.clone(),
            title,
            status,
            path: repo_path(&path),
        };
        if docs.insert(id.clone(), doc).is_some() {
            bail!("duplicate behavior id {id}");
        }
    }
    Ok(docs)
}

fn validate_behavior_docs(docs: &BTreeMap<String, BehaviorDoc>) -> Vec<String> {
    let mut errors = Vec::new();
    for (id, doc) in docs {
        if !valid_behavior_id(id) {
            errors.push(format!("{}: invalid behavior id {}", doc.path, doc.id));
        }
        if doc.title.trim().is_empty() {
            errors.push(format!("{}: missing title", doc.path));
        }
        if !matches!(doc.status.as_str(), "draft" | "accepted" | "deprecated") {
            errors.push(format!("{}: invalid status {}", doc.path, doc.status));
        }
        let Some(file_id) = Path::new(&doc.path)
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.get(0..6))
        else {
            errors.push(format!(
                "{}: filename must start with behavior id",
                doc.path
            ));
            continue;
        };
        if file_id != id {
            errors.push(format!(
                "{}: filename id {file_id} does not match frontmatter id {id}",
                doc.path
            ));
        }
    }
    errors
}

fn load_feature_scenarios(
    root: &Path,
    docs: &BTreeMap<String, BehaviorDoc>,
    errors: &mut Vec<String>,
) -> Result<Vec<FeatureScenario>> {
    let mut files = Vec::new();
    collect_feature_files(root, &mut files)?;
    files.sort();
    let mut scenarios = Vec::new();
    for path in files {
        parse_feature_file(&path, docs, errors, &mut scenarios)?;
    }
    Ok(scenarios)
}

fn parse_feature_file(
    path: &Path,
    docs: &BTreeMap<String, BehaviorDoc>,
    errors: &mut Vec<String>,
    scenarios: &mut Vec<FeatureScenario>,
) -> Result<()> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let file = repo_path(path);
    let mut feature_tags = Vec::new();
    let mut pending_tags = Vec::new();
    let mut in_feature = false;
    for (index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.starts_with('@') {
            let tags = line
                .split_whitespace()
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            if in_feature {
                pending_tags = tags;
            } else {
                feature_tags = tags;
            }
            continue;
        }
        if line.starts_with("Feature:") {
            in_feature = true;
            continue;
        }
        let title = line
            .strip_prefix("Scenario:")
            .or_else(|| line.strip_prefix("Scenario Outline:"));
        let Some(title) = title else {
            continue;
        };
        let mut tags = feature_tags.clone();
        tags.extend(pending_tags.clone());
        pending_tags.clear();
        validate_scenario_tags(&file, index + 1, &tags, docs, errors);
        let behavior_ids = tags
            .iter()
            .filter_map(|tag| tag.strip_prefix('@'))
            .filter(|tag| valid_behavior_id(tag))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let witnesses = tags
            .iter()
            .filter_map(|tag| match tag.as_str() {
                "@positive" => Some("positive".to_string()),
                "@falsification" => Some("falsification".to_string()),
                _ => None,
            })
            .collect::<Vec<_>>();
        if behavior_ids.len() == 1 && witnesses.len() == 1 {
            scenarios.push(FeatureScenario {
                behavior_id: behavior_ids[0].clone(),
                witness: witnesses[0].clone(),
                title: title.trim().to_string(),
                feature_file: file.clone(),
                line: index + 1,
                tags,
            });
        }
    }
    Ok(())
}

fn validate_scenario_tags(
    file: &str,
    line: usize,
    tags: &[String],
    docs: &BTreeMap<String, BehaviorDoc>,
    errors: &mut Vec<String>,
) {
    for tag in tags {
        let bare = tag.trim_start_matches('@');
        if tag == "@phase0"
            || bare.starts_with("wave_")
            || bare.starts_with("BEH-P0-")
            || matches!(
                tag.as_str(),
                "@skip" | "@skipped" | "@ignore" | "@ignored" | "@pending" | "@wip"
            )
        {
            errors.push(format!("{file}:{line}: forbidden scenario tag {tag}"));
        }
    }
    let behavior_ids = tags
        .iter()
        .filter_map(|tag| tag.strip_prefix('@'))
        .filter(|tag| valid_behavior_id(tag))
        .collect::<Vec<_>>();
    if behavior_ids.len() != 1 {
        errors.push(format!(
            "{file}:{line}: scenario must reference exactly one @B-XXXX tag"
        ));
        return;
    }
    let behavior_id = behavior_ids[0];
    let Some(doc) = docs.get(behavior_id) else {
        errors.push(format!(
            "{file}:{line}: behavior id {behavior_id} has no matching behavior doc"
        ));
        return;
    };
    if doc.status == "deprecated" && !has_deprecated_coverage_tag(tags) {
        errors.push(format!(
            "{file}:{line}: deprecated behavior {behavior_id} requires compatibility, migration, or deprecation coverage tag"
        ));
    }
    let witness_count = tags
        .iter()
        .filter(|tag| matches!(tag.as_str(), "@positive" | "@falsification"))
        .count();
    if witness_count != 1 {
        errors.push(format!(
            "{file}:{line}: scenario must declare exactly one witness tag"
        ));
    }
}

fn validate_witness_obligations(
    docs: &BTreeMap<String, BehaviorDoc>,
    scenarios: &[FeatureScenario],
    errors: &mut Vec<String>,
) {
    for doc in docs.values().filter(|doc| doc.status == "accepted") {
        let has_positive = scenarios
            .iter()
            .any(|scenario| scenario.behavior_id == doc.id && scenario.witness == "positive");
        let has_falsification = scenarios
            .iter()
            .any(|scenario| scenario.behavior_id == doc.id && scenario.witness == "falsification");
        if !has_positive {
            errors.push(format!(
                "{}: accepted behavior missing positive witness",
                doc.id
            ));
        }
        if !has_falsification {
            errors.push(format!(
                "{}: accepted behavior missing falsification witness",
                doc.id
            ));
        }
    }
}

fn behavior_inventory_json(inventory: &BehaviorInventory) -> Value {
    json!({
        "schema_version": "1.0.0",
        "artifact": "behavior_inventory",
        "generated_at": now_timestamp(),
        "behavior_docs": inventory.docs.values().map(|doc| {
            json!({
                "id": doc.id,
                "title": doc.title,
                "status": doc.status,
                "path": doc.path,
            })
        }).collect::<Vec<_>>(),
        "scenarios": inventory.scenarios.iter().map(|scenario| {
            json!({
                "behavior_id": scenario.behavior_id,
                "witness": scenario.witness,
                "title": scenario.title,
                "feature_file": scenario.feature_file,
                "line": scenario.line,
                "tags": scenario.tags,
            })
        }).collect::<Vec<_>>(),
        "witness_summary": behavior_witness_summary(inventory),
        "errors": inventory.errors,
    })
}

fn behavior_witness_summary(inventory: &BehaviorInventory) -> Vec<Value> {
    inventory
        .docs
        .values()
        .filter(|doc| doc.status == "accepted")
        .map(|doc| {
            let positive = inventory
                .scenarios
                .iter()
                .filter(|scenario| scenario.behavior_id == doc.id && scenario.witness == "positive")
                .count();
            let falsification = inventory
                .scenarios
                .iter()
                .filter(|scenario| {
                    scenario.behavior_id == doc.id && scenario.witness == "falsification"
                })
                .count();
            json!({
                "behavior_id": doc.id,
                "title": doc.title,
                "positive_witnesses": positive,
                "falsification_witnesses": falsification,
                "classification": if positive > 0 && falsification > 0 {
                    "covered_behavior"
                } else if positive == 0 {
                    "missing_positive_witness"
                } else {
                    "missing_falsification_witness"
                },
            })
        })
        .collect()
}

fn accepted_behavior_count(inventory: &BehaviorInventory) -> usize {
    inventory
        .docs
        .values()
        .filter(|doc| doc.status == "accepted")
        .count()
}

fn print_behavior_errors(errors: &[String]) {
    println!("Behavior validation failed:");
    for error in errors {
        println!("- {error}");
    }
}

fn parse_frontmatter(text: &str) -> Result<serde_yaml::Value> {
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        bail!("missing opening frontmatter delimiter");
    }
    let mut yaml = String::new();
    for line in lines {
        if line == "---" {
            return serde_yaml::from_str(&yaml).context("parse behavior frontmatter");
        }
        yaml.push_str(line);
        yaml.push('\n');
    }
    bail!("missing closing frontmatter delimiter")
}

fn yaml_string(value: &serde_yaml::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_yaml::Value::as_str)
        .map(ToOwned::to_owned)
}

fn collect_markdown_files(path: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let entry = entry?;
        let child = entry.path();
        if child.is_dir() {
            collect_markdown_files(&child, out)?;
        } else if child.extension().is_some_and(|ext| ext == "md") {
            out.push(child);
        }
    }
    Ok(())
}

fn collect_feature_files(path: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !path.exists() {
        bail!("missing BDD feature directory {}", path.display());
    }
    for entry in fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let entry = entry?;
        let child = entry.path();
        if child.is_dir() {
            collect_feature_files(&child, out)?;
        } else if child.extension().is_some_and(|ext| ext == "feature") {
            out.push(child);
        }
    }
    Ok(())
}

fn has_deprecated_coverage_tag(tags: &[String]) -> bool {
    tags.iter().any(|tag| {
        matches!(
            tag.as_str(),
            "@compatibility" | "@migration" | "@deprecation"
        )
    })
}

fn timestamped_dir(root: &str) -> Result<PathBuf> {
    let stamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let path = Path::new(root).join(stamp);
    fs::create_dir_all(&path).with_context(|| format!("create {}", path.display()))?;
    Ok(path)
}

fn update_latest(root: &Path, run_dir: &Path) -> Result<()> {
    fs::create_dir_all(root).with_context(|| format!("create {}", root.display()))?;
    let latest = root.join("latest");
    if latest.exists() || latest.symlink_metadata().is_ok() {
        fs::remove_file(&latest).or_else(|_| fs::remove_dir_all(&latest))?;
    }
    let Some(target) = run_dir.file_name() else {
        bail!("invalid run dir {}", run_dir.display());
    };
    std::os::unix::fs::symlink(target, &latest)
        .with_context(|| format!("update latest symlink {}", latest.display()))
}

fn target_debug_binary(name: &str) -> PathBuf {
    let mut path = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target"));
    path.push("debug");
    path.push(name);
    path
}

fn absolute_path(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        env::current_dir()
            .context("read current directory")
            .map(|cwd| cwd.join(path))
    }
}

fn run_command(command: &mut Command, label: &str) -> Result<()> {
    let status = command.status().with_context(|| format!("run {label}"))?;
    if !status.success() {
        bail!("{label} failed with status {status}");
    }
    Ok(())
}

fn run_command_capture(
    command: &mut Command,
    stdout: &Path,
    stderr: &Path,
) -> Result<std::process::ExitStatus> {
    let output = command.output().context("run command")?;
    fs::write(stdout, &output.stdout).with_context(|| format!("write {}", stdout.display()))?;
    fs::write(stderr, &output.stderr).with_context(|| format!("write {}", stderr.display()))?;
    Ok(output.status)
}

fn write_command_file(path: &Path, command: &Command) -> Result<()> {
    fs::write(path, format!("{command:?}\n")).with_context(|| format!("write {}", path.display()))
}

fn uncovered_product_code(workspace_by_file: &BTreeMap<String, Value>) -> Vec<Value> {
    workspace_by_file
        .values()
        .filter(|row| {
            let path = row.get("file").and_then(Value::as_str).unwrap_or_default();
            let covered = row
                .get("lines")
                .and_then(Value::as_object)
                .and_then(|lines| lines.get("covered"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            covered == 0
                && !path.starts_with("crates/tanren-bdd/")
                && !path.starts_with("crates/tanren-testkit/")
                && !path.starts_with("xtask/")
        })
        .map(|row| {
            let path = row.get("file").and_then(Value::as_str).unwrap_or_default();
            json!({
                "classification": "uncovered_product_code",
                "file": path,
                "recommendation": "add behavior coverage, classify as support code, or remove as dead code",
            })
        })
        .collect()
}

fn check_rust_test_surface() -> Result<()> {
    let mut errors = Vec::new();

    let test_files = rust_test_tree_files()?;
    if !test_files.is_empty() {
        errors.push(format!(
            "non-BDD Rust test-tree files remain:\n{}",
            bullet_list(&test_files)
        ));
    }

    let cfg_files = cfg_test_offenders()?;
    if !cfg_files.is_empty() {
        errors.push(format!(
            "non-BDD #[cfg(test)]/mod tests remain:\n{}",
            bullet_list(&cfg_files)
        ));
    }

    let policy_refs = forbidden_policy_references()?;
    if !policy_refs.is_empty() {
        errors.push(format!(
            "retired test contract references remain:\n{}",
            bullet_list(&policy_refs)
        ));
    }

    if !errors.is_empty() {
        println!("Rust test surface contract failed.");
        println!("{}", errors.join("\n\n"));
        bail!("rust test surface contract failed");
    }
    Ok(())
}

fn rust_test_tree_files() -> Result<Vec<String>> {
    let mut offenders = Vec::new();
    for base in ["crates", "bin"] {
        collect_test_tree_files(Path::new(base), &mut offenders)?;
    }
    offenders.sort();
    Ok(offenders)
}

fn collect_test_tree_files(path: &Path, offenders: &mut Vec<String>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let entry = entry?;
        let child = entry.path();
        if child.is_dir() {
            if child.file_name().is_some_and(|name| name == "tests") {
                collect_files(&child, offenders)?;
            } else {
                collect_test_tree_files(&child, offenders)?;
            }
        }
    }
    Ok(())
}

fn collect_files(path: &Path, out: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let entry = entry?;
        let child = entry.path();
        if child.is_dir() {
            collect_files(&child, out)?;
        } else if child.is_file() {
            out.push(repo_path(&child));
        }
    }
    Ok(())
}

fn cfg_test_offenders() -> Result<Vec<String>> {
    let mut offenders = Vec::new();
    for base in ["crates", "bin"] {
        collect_cfg_test_offenders(Path::new(base), &mut offenders)?;
    }
    offenders.sort();
    Ok(offenders)
}

fn collect_cfg_test_offenders(path: &Path, offenders: &mut Vec<String>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let entry = entry?;
        let child = entry.path();
        if child.is_dir() {
            collect_cfg_test_offenders(&child, offenders)?;
            continue;
        }
        if child.extension().is_some_and(|ext| ext == "rs")
            && !repo_path(&child).starts_with(BDD_CRATE)
        {
            let text =
                fs::read_to_string(&child).with_context(|| format!("read {}", child.display()))?;
            if text.contains("#[cfg(test)]")
                || text.contains("mod tests;")
                || text.contains("mod tests {")
            {
                offenders.push(repo_path(&child));
            }
        }
    }
    Ok(())
}

fn forbidden_policy_references() -> Result<Vec<String>> {
    let mut offenders = Vec::new();
    for path_str in CURRENT_POLICY_FILES {
        let path = Path::new(path_str);
        if !path.exists() {
            continue;
        }
        let text = fs::read_to_string(path).with_context(|| format!("read {path_str}"))?;
        for snippet in FORBIDDEN_POLICY_SNIPPETS {
            if text.contains(snippet) {
                offenders.push(format!("{path_str}: {snippet}"));
            }
        }
    }
    Ok(offenders)
}

fn render_mutation_triage(raw: &[String]) -> Result<()> {
    let args = Args::parse(raw)?;
    let traceability = args.required_path("traceability")?;
    let run_dir = args.required_path("run-dir")?;
    let mutants_out = args.required_path("mutants-out")?;
    let status = args.required_string("status")?;
    let exit_code = args.required_i32("exit-code")?;
    let version = args.string_or_empty("version");
    let shard = args.required_string("shard")?;

    fs::create_dir_all(&run_dir).with_context(|| format!("create {}", run_dir.display()))?;
    let (wave_ids, all_ids) = load_wave_behavior_ids(&traceability)?;
    let resolved_mutants_out = resolve_mutants_out_dir(&mutants_out);
    let survivors = build_survivors(&resolved_mutants_out, &wave_ids, &all_ids)?;
    let counts = count_outcomes(&resolved_mutants_out)?;

    let report = json!({
        "schema_version": "1.0.0",
        "artifact": "mutation_triage",
        "generated_at": now_timestamp(),
        "stage": "staged_non_blocking",
        "run": {
            "status": status,
            "exit_code": exit_code,
            "cargo_mutants_version": version,
            "shard": shard,
            "package": "tanren-bdd",
            "test_package": "tanren-bdd",
            "files": [BEHAVIOR_SOURCE_A, BEHAVIOR_SOURCE_B, BEHAVIOR_SOURCE_C],
            "baseline": "skip",
            "non_blocking": true,
        },
        "outcomes": counts,
        "survivors": serialize_survivors(&survivors),
        "policy": {
            "survivor_triage": [
                "Every missed/timeout mutant must be linked to at least one BEH-* id.",
                "Resolve by tightening an existing scenario, adding a falsification scenario, or marking equivalent with explicit justification.",
                "Unmapped survivors must be escalated before final enforcement."
            ],
            "linkage_sources": {
                "traceability_inventory": traceability.display().to_string(),
                "wave_mappings": {
                    "A": wave_ids.get("A").cloned().unwrap_or_default(),
                    "B": wave_ids.get("B").cloned().unwrap_or_default(),
                    "C": wave_ids.get("C").cloned().unwrap_or_default(),
                }
            }
        },
        "artifacts": {
            "command": run_dir.join("command.txt").display().to_string(),
            "stdout": run_dir.join("cargo-mutants.stdout.log").display().to_string(),
            "stderr": run_dir.join("cargo-mutants.stderr.log").display().to_string(),
            "requested_mutants_out": mutants_out.display().to_string(),
            "mutants_out": resolved_mutants_out.display().to_string(),
            "mutants_json": resolved_mutants_out.join("mutants.json").display().to_string(),
            "outcomes_json": resolved_mutants_out.join("outcomes.json").display().to_string(),
            "caught_txt": resolved_mutants_out.join("caught.txt").display().to_string(),
            "missed_txt": resolved_mutants_out.join("missed.txt").display().to_string(),
            "timeout_txt": resolved_mutants_out.join("timeout.txt").display().to_string(),
            "unviable_txt": resolved_mutants_out.join("unviable.txt").display().to_string(),
        }
    });
    write_json(&run_dir.join("triage.json"), &report)
}

fn render_coverage_classification(raw: &[String]) -> Result<()> {
    let args = Args::parse(raw)?;
    let traceability = args.required_path("traceability")?;
    let run_dir = args.required_path("run-dir")?;
    let coverage_summary = args.required_path("coverage-summary")?;
    let feature_executions_path = args.required_path("feature-executions")?;
    let status = args.required_string("status")?;
    let exit_code = args.required_i32("exit-code")?;
    let version = args.string_or_empty("version");

    fs::create_dir_all(&run_dir).with_context(|| format!("create {}", run_dir.display()))?;
    let coverage_payload = load_json_or_empty(&coverage_summary)?;
    let (coverage_by_file, llvm_totals) = load_coverage_summary(&coverage_payload)?;
    let source_lines_by_file = workspace_rust_sources(Path::new("."))?;
    let (workspace_by_file, workspace_totals) =
        build_workspace_coverage(&coverage_by_file, &source_lines_by_file);
    let feature_executions = load_feature_executions(&feature_executions_path)?;
    let behavior_inventory = load_behavior_inventory(&traceability)?;
    let (covered_behaviors, missing_behaviors) =
        classify_behavior_coverage(&behavior_inventory, &feature_executions, &coverage_by_file);
    let (non_behavior_code, unmeasured_workspace_code) =
        classify_workspace_code(&workspace_by_file);

    let report = json!({
        "schema_version": "1.0.0",
        "artifact": "coverage_classification",
        "generated_at": now_timestamp(),
        "stage": "staged_non_blocking",
        "run": {
            "status": status,
            "exit_code": exit_code,
            "cargo_llvm_cov_version": version,
            "package": "tanren-bdd",
            "bin": "tanren-bdd",
            "non_blocking": true,
            "coverage_scope": "workspace_rust_source",
        },
        "behavior_inventory": {
            "source": traceability.display().to_string(),
            "total_count": behavior_inventory.len(),
            "covered_count": covered_behaviors.len(),
            "missing_count": missing_behaviors.len(),
            "covered": covered_behaviors,
            "missing": missing_behaviors,
        },
        "classification": {
            "missing_behavior": missing_behaviors,
            "non_behavior_code": non_behavior_code,
            "unmeasured_workspace_code": unmeasured_workspace_code,
        },
        "coverage_summary": {
            "workspace_totals": workspace_totals,
            "llvm_measured_totals": llvm_totals,
            "workspace_files": files_payload(&workspace_by_file),
            "llvm_measured_files": files_payload(&coverage_by_file),
            "behavior_owner_sources": [BEHAVIOR_SOURCE_A, BEHAVIOR_SOURCE_B, BEHAVIOR_SOURCE_C],
        },
        "feature_executions": feature_executions_payload(&feature_executions),
        "policy": {
            "coverage_gate": [
                "Coverage execution and artifact generation are required.",
                "No minimum workspace percentage is enforced yet.",
                "The CI byline reports coverage_summary.workspace_totals, not package-only llvm totals."
            ],
            "behavior_gap_triage": [
                "Any behavior listed under classification.missing_behavior requires scenario coverage remediation before final enforcement.",
                "Feature execution failures take precedence over source-level percentages when classifying missing behavior."
            ],
            "unmeasured_code_triage": [
                "Unmeasured workspace Rust files are counted as zero-covered in workspace_totals.",
                "Move BDD steps onto real runtime code to convert unmeasured files into measured coverage."
            ]
        },
        "artifacts": {
            "command": run_dir.join("command.txt").display().to_string(),
            "feature_executions": feature_executions_path.display().to_string(),
            "coverage_summary_json": coverage_summary.display().to_string(),
            "lcov": run_dir.join("lcov.info").display().to_string(),
            "coverage_run_stdout": run_dir.join("coverage-run.stdout.log").display().to_string(),
            "coverage_run_stderr": run_dir.join("coverage-run.stderr.log").display().to_string(),
            "coverage_lcov_stdout": run_dir.join("coverage-lcov.stdout.log").display().to_string(),
            "coverage_lcov_stderr": run_dir.join("coverage-lcov.stderr.log").display().to_string(),
        }
    });
    write_json(&run_dir.join("classification.json"), &report)
}

fn valid_behavior_id(id: &str) -> bool {
    id.len() == 6 && id.starts_with("B-") && id[2..].chars().all(|c| c.is_ascii_digit())
}

#[derive(Debug, Clone)]
struct SurvivorRecord {
    outcome: String,
    mutant: String,
    source_path: Option<String>,
    source_line: Option<u64>,
    behavior_ids: Vec<String>,
    linkage_mode: String,
}

fn load_wave_behavior_ids(path: &Path) -> Result<(WaveBehaviorIds, Vec<String>)> {
    let payload = load_json(path)?;
    let mut by_wave: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut all_ids = BTreeSet::new();
    for row in payload
        .get("behavior_inventory")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(object) = row.as_object() else {
            continue;
        };
        let Some(behavior_id) = object.get("behavior_id").and_then(Value::as_str) else {
            continue;
        };
        all_ids.insert(behavior_id.to_string());
        if let Some(wave) = object.get("wave").and_then(Value::as_str) {
            by_wave
                .entry(wave.to_string())
                .or_default()
                .insert(behavior_id.to_string());
        }
    }
    let by_wave = by_wave
        .into_iter()
        .map(|(wave, ids)| (wave, ids.into_iter().collect::<Vec<_>>()))
        .collect::<BTreeMap<_, _>>();
    Ok((by_wave, all_ids.into_iter().collect()))
}

fn resolve_mutants_out_dir(mutants_out: &Path) -> PathBuf {
    for candidate in [mutants_out.to_path_buf(), mutants_out.join("mutants.out")] {
        if candidate.join("outcomes.json").exists()
            || SUMMARY_OUTCOME_FILES
                .iter()
                .any(|(_, filename)| candidate.join(filename).exists())
        {
            return candidate;
        }
    }
    mutants_out.to_path_buf()
}

fn build_survivors(
    mutants_out: &Path,
    wave_ids: &BTreeMap<String, Vec<String>>,
    all_ids: &[String],
) -> Result<Vec<SurvivorRecord>> {
    let mut dedupe = BTreeSet::new();
    let mut rows = Vec::new();

    if let Some(payload) = load_json_optional(&mutants_out.join("outcomes.json"))? {
        for row in payload
            .get("outcomes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(outcome) = row
                .get("summary")
                .and_then(Value::as_str)
                .map(|summary| summary.trim().to_ascii_lowercase())
            else {
                continue;
            };
            if outcome != "missed" && outcome != "timeout" {
                continue;
            }
            let Some(mutant) = mutant_label_from_outcome(row) else {
                continue;
            };
            if dedupe.insert((outcome.clone(), mutant.clone())) {
                rows.push(survivor_record(outcome, mutant, wave_ids, all_ids));
            }
        }
    }

    for (outcome, filename) in SURVIVOR_OUTCOME_FILES {
        for mutant in read_lines(&mutants_out.join(filename))? {
            if dedupe.insert(((*outcome).to_string(), mutant.clone())) {
                rows.push(survivor_record(
                    (*outcome).to_string(),
                    mutant,
                    wave_ids,
                    all_ids,
                ));
            }
        }
    }
    Ok(rows)
}

fn survivor_record(
    outcome: String,
    mutant: String,
    wave_ids: &BTreeMap<String, Vec<String>>,
    all_ids: &[String],
) -> SurvivorRecord {
    let (source_path, source_line) = parse_source_ref(&mutant);
    let (behavior_ids, linkage_mode) = map_behavior_ids(source_path.as_deref(), wave_ids, all_ids);
    SurvivorRecord {
        outcome,
        mutant,
        source_path,
        source_line,
        behavior_ids,
        linkage_mode,
    }
}

fn mutant_label_from_outcome(row: &Value) -> Option<String> {
    row.get("scenario")
        .and_then(|scenario| scenario.get("Mutant"))
        .and_then(|mutant| mutant.get("name"))
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(|name| name.trim().to_string())
        .or_else(|| {
            row.get("mutant")
                .and_then(Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .map(|name| name.trim().to_string())
        })
}

fn parse_source_ref(mutant_label: &str) -> (Option<String>, Option<u64>) {
    let Some(rs_index) = mutant_label.find(".rs:") else {
        return (None, None);
    };
    let path_end = rs_index + 3;
    let path_start = mutant_label[..path_end]
        .rfind(char::is_whitespace)
        .map_or(0, |idx| idx + 1);
    let path = mutant_label[path_start..path_end].to_string();
    let line_start = path_end + 1;
    let line_digits = mutant_label[line_start..]
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    let line = line_digits.parse::<u64>().ok();
    (Some(path), line)
}

fn map_behavior_ids(
    source_path: Option<&str>,
    wave_ids: &BTreeMap<String, Vec<String>>,
    all_ids: &[String],
) -> (Vec<String>, String) {
    let Some(source_path) = source_path else {
        return (Vec::new(), "unmapped".to_string());
    };
    let normalized = source_path.replace('\\', "/");
    if normalized.ends_with(BEHAVIOR_SOURCE_B) {
        return (
            wave_ids.get("B").cloned().unwrap_or_default(),
            "wave_map".to_string(),
        );
    }
    if normalized.ends_with(BEHAVIOR_SOURCE_C) {
        return (
            wave_ids.get("C").cloned().unwrap_or_default(),
            "wave_map".to_string(),
        );
    }
    if normalized.ends_with(BEHAVIOR_SOURCE_A) {
        return (
            wave_ids
                .get("A")
                .cloned()
                .unwrap_or_else(|| all_ids.to_vec()),
            "wave_map".to_string(),
        );
    }
    if normalized.contains("crates/tanren-bdd/src/") {
        return (all_ids.to_vec(), "crate_coarse".to_string());
    }
    (Vec::new(), "unmapped".to_string())
}

fn count_outcomes(mutants_out: &Path) -> Result<Value> {
    if let Some(payload) = load_json_optional(&mutants_out.join("outcomes.json"))? {
        let caught = parse_count(payload.get("caught"));
        let missed = parse_count(payload.get("missed"));
        let timeout = parse_count(payload.get("timeout"));
        let unviable = parse_count(payload.get("unviable"));
        let total = parse_count(payload.get("total_mutants"));
        return Ok(json!({
            "caught_count": caught,
            "missed_count": missed,
            "timeout_count": timeout,
            "unviable_count": unviable,
            "tested_count": if total > 0 { total } else { caught + missed + timeout + unviable },
        }));
    }

    let mut counts = BTreeMap::new();
    let mut total = 0_u64;
    for (outcome, filename) in SUMMARY_OUTCOME_FILES {
        let count = read_lines(&mutants_out.join(filename))?.len() as u64;
        total += count;
        counts.insert(format!("{outcome}_count"), count);
    }
    counts.insert("tested_count".to_string(), total);
    Ok(json!(counts))
}

fn parse_count(value: Option<&Value>) -> u64 {
    match value {
        Some(Value::Number(number)) => number.as_u64().unwrap_or(0),
        Some(Value::String(text)) => text.parse::<u64>().unwrap_or(0),
        _ => 0,
    }
}

fn serialize_survivors(rows: &[SurvivorRecord]) -> Value {
    Value::Array(
        rows.iter()
            .map(|row| {
                let recommended_action = if row.outcome == "missed" {
                    "tighten_or_add_behavior_scenario"
                } else {
                    "investigate_timeout_and_record_behavior_link"
                };
                json!({
                    "outcome": row.outcome,
                    "mutant": row.mutant,
                    "source_path": row.source_path,
                    "source_line": row.source_line,
                    "behavior_ids": row.behavior_ids,
                    "linkage_mode": row.linkage_mode,
                    "triage_required": row.outcome == "missed" || row.outcome == "timeout",
                    "recommended_action": recommended_action,
                })
            })
            .collect(),
    )
}

fn load_coverage_summary(payload: &Value) -> Result<(BTreeMap<String, Value>, Value)> {
    let Some(head) = payload
        .get("data")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(Value::as_object)
    else {
        return Ok((BTreeMap::new(), json!({})));
    };
    let totals = head.get("totals").cloned().unwrap_or_else(|| json!({}));
    let mut by_file = BTreeMap::new();
    for row in head
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(filename) = row.get("filename").and_then(Value::as_str) else {
            continue;
        };
        let Some(summary) = row.get("summary").and_then(Value::as_object) else {
            continue;
        };
        let relpath = normalize_path(filename)?;
        by_file.insert(
            relpath.clone(),
            json!({
                "path": relpath,
                "measured_by_llvm": true,
                "lines": metric(summary.get("lines")),
                "functions": metric(summary.get("functions")),
                "regions": metric(summary.get("regions")),
            }),
        );
    }
    Ok((by_file, totals))
}

fn metric(value: Option<&Value>) -> Value {
    let object = value.and_then(Value::as_object);
    json!({
        "count": number_field(object, "count"),
        "covered": number_field(object, "covered"),
        "percent": float_field(object, "percent"),
    })
}

fn workspace_rust_sources(repo_root: &Path) -> Result<BTreeMap<String, u64>> {
    let mut rows = BTreeMap::new();
    for root_name in ["crates", "bin"] {
        let root = repo_root.join(root_name);
        collect_rust_sources(&root, &mut rows)?;
    }
    Ok(rows)
}

fn collect_rust_sources(path: &Path, rows: &mut BTreeMap<String, u64>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let entry = entry?;
        let child = entry.path();
        if child.is_dir() {
            if child.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            collect_rust_sources(&child, rows)?;
        } else if child.extension().is_some_and(|ext| ext == "rs") {
            rows.insert(repo_path(&child), rust_source_line_count(&child)?);
        }
    }
    Ok(())
}

fn rust_source_line_count(path: &Path) -> Result<u64> {
    let mut count = 0;
    let mut in_block_comment = false;
    for raw_line in fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?
        .lines()
    {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if in_block_comment {
            if let Some((_, tail)) = line.split_once("*/") {
                in_block_comment = false;
                let tail = tail.trim();
                if !tail.is_empty() && !tail.starts_with("//") {
                    count += 1;
                }
            }
            continue;
        }
        if line.starts_with("/*") || line.starts_with("//!") || line.starts_with("///") {
            if line.starts_with("/*") && !line.contains("*/") {
                in_block_comment = true;
            }
            continue;
        }
        if line.starts_with("//") {
            continue;
        }
        count += 1;
    }
    Ok(count)
}

fn build_workspace_coverage(
    coverage_by_file: &BTreeMap<String, Value>,
    source_lines_by_file: &BTreeMap<String, u64>,
) -> (BTreeMap<String, Value>, Value) {
    let mut rows = BTreeMap::new();
    let mut measured_count = 0_u64;
    let mut total_lines = 0_u64;
    let mut covered_lines = 0_u64;

    for (path, source_line_count) in source_lines_by_file {
        let measured = coverage_by_file.get(path);
        let (line_count, line_covered, measurement) = if let Some(measured) = measured {
            measured_count += 1;
            let measured_line_count = measured
                .get("lines")
                .and_then(|lines| lines.get("count"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let measured_covered = measured
                .get("lines")
                .and_then(|lines| lines.get("covered"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            (
                if measured_line_count == 0 {
                    *source_line_count
                } else {
                    measured_line_count
                },
                measured_covered,
                "llvm",
            )
        } else {
            (*source_line_count, 0, "source_inventory_zero_covered")
        };
        total_lines += line_count;
        covered_lines += line_covered;
        rows.insert(
            path.clone(),
            json!({
                "path": path,
                "measured_by_llvm": measured.is_some(),
                "measurement": measurement,
                "source_line_count": source_line_count,
                "lines": {
                    "count": line_count,
                    "covered": line_covered,
                    "percent": percent(line_covered, line_count),
                }
            }),
        );
    }

    let totals = json!({
        "lines": {
            "count": total_lines,
            "covered": covered_lines,
            "uncovered": total_lines.saturating_sub(covered_lines),
            "percent": percent(covered_lines, total_lines),
        },
        "files": {
            "count": source_lines_by_file.len(),
            "measured_count": measured_count,
            "unmeasured_count": (source_lines_by_file.len() as u64).saturating_sub(measured_count),
        },
        "definition": "All Rust source files under crates/ and bin/; files absent from llvm-cov output are counted as zero-covered using non-blank non-comment source lines.",
    });
    (rows, totals)
}

fn load_feature_executions(path: &Path) -> Result<BTreeMap<String, Value>> {
    let mut rows = BTreeMap::new();
    if !path.exists() {
        return Ok(rows);
    }
    for line in fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?
        .lines()
    {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(row) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(feature_file) = row.get("feature_file").and_then(Value::as_str) else {
            continue;
        };
        rows.insert(
            feature_file.to_string(),
            json!({
                "status": row.get("status").and_then(Value::as_str).unwrap_or("unknown"),
                "exit_code": row.get("exit_code").and_then(Value::as_i64).unwrap_or(0),
            }),
        );
    }
    Ok(rows)
}

fn load_behavior_inventory(path: &Path) -> Result<Vec<Value>> {
    let payload = load_json(path)?;
    let mut rows = Vec::new();
    for row in payload
        .get("behavior_inventory")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(object) = row.as_object() else {
            continue;
        };
        let Some(behavior_id) = object.get("behavior_id").and_then(Value::as_str) else {
            continue;
        };
        let scenario_id = object
            .get("scenario_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let wave = object
            .get("wave")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let planned_feature_file = object
            .get("obligations")
            .and_then(|obligations| obligations.get("positive"))
            .and_then(|positive| positive.get("planned_feature_file"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        rows.push(json!({
            "behavior_id": behavior_id,
            "scenario_id": scenario_id,
            "wave": wave,
            "planned_feature_file": planned_feature_file,
        }));
    }
    Ok(rows)
}

fn classify_behavior_coverage(
    inventory: &[Value],
    feature_executions: &BTreeMap<String, Value>,
    coverage_by_file: &BTreeMap<String, Value>,
) -> (Vec<Value>, Vec<Value>) {
    let mut covered = Vec::new();
    let mut missing = Vec::new();

    for row in inventory {
        let behavior_id = row
            .get("behavior_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let scenario_id = row
            .get("scenario_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let wave = row.get("wave").and_then(Value::as_str).unwrap_or_default();
        let feature_path = row
            .get("planned_feature_file")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let source_path = wave_source(wave);
        let feature_exec = feature_executions.get(feature_path);
        let feature_status = feature_exec
            .and_then(|feature| feature.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("not_executed");
        let feature_exit_code = feature_exec
            .and_then(|feature| feature.get("exit_code"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let source_cov = source_path.and_then(|source| coverage_by_file.get(source));
        let source_line_percent = source_cov
            .and_then(|source| source.get("lines"))
            .and_then(|lines| lines.get("percent"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let source_lines_covered = source_cov
            .and_then(|source| source.get("lines"))
            .and_then(|lines| lines.get("covered"))
            .and_then(Value::as_u64)
            .unwrap_or(0);

        let mut entry = Map::new();
        entry.insert("behavior_id".to_string(), json!(behavior_id));
        entry.insert("scenario_id".to_string(), json!(scenario_id));
        entry.insert("wave".to_string(), json!(wave));
        entry.insert("planned_feature_file".to_string(), json!(feature_path));
        entry.insert("feature_status".to_string(), json!(feature_status));
        entry.insert("feature_exit_code".to_string(), json!(feature_exit_code));
        entry.insert(
            "source_path".to_string(),
            source_path.map_or(Value::Null, |source| json!(source)),
        );
        entry.insert(
            "source_line_percent".to_string(),
            json!(source_line_percent),
        );

        let reason = if feature_status != "passed" {
            Some("planned_feature_not_passing")
        } else if source_path.is_none() {
            Some("wave_has_no_source_mapping")
        } else if source_cov.is_none() {
            Some("source_missing_from_coverage_report")
        } else if source_lines_covered == 0 {
            Some("source_has_zero_covered_lines")
        } else {
            None
        };

        if let Some(reason) = reason {
            entry.insert("classification".to_string(), json!("missing_behavior"));
            entry.insert("reason".to_string(), json!(reason));
            missing.push(Value::Object(entry));
        } else {
            entry.insert("classification".to_string(), json!("covered_behavior"));
            entry.insert(
                "reason".to_string(),
                json!("planned_feature_passed_and_wave_source_covered"),
            );
            covered.push(Value::Object(entry));
        }
    }
    (covered, missing)
}

fn classify_workspace_code(
    workspace_by_file: &BTreeMap<String, Value>,
) -> (Vec<Value>, Vec<Value>) {
    let behavior_sources = [BEHAVIOR_SOURCE_A, BEHAVIOR_SOURCE_B, BEHAVIOR_SOURCE_C]
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut non_behavior = Vec::new();
    let mut unmeasured = Vec::new();
    for (path, summary) in workspace_by_file {
        if behavior_sources.contains(path.as_str()) {
            continue;
        }
        let line_percent = summary
            .get("lines")
            .and_then(|lines| lines.get("percent"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let line_count = summary
            .get("lines")
            .and_then(|lines| lines.get("count"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let measured = summary
            .get("measured_by_llvm")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let (classification, rationale) = if measured {
            (
                "support_code_measured",
                "file is unmapped to behavior inventory but present in llvm-cov output",
            )
        } else {
            (
                "unmeasured_workspace_code",
                "file is workspace Rust source absent from llvm-cov output and counted as zero-covered",
            )
        };
        let row = json!({
            "path": path,
            "classification": classification,
            "line_percent": line_percent,
            "line_count": line_count,
            "rationale": rationale,
        });
        if !measured {
            unmeasured.push(row.clone());
        }
        non_behavior.push(row);
    }
    (non_behavior, unmeasured)
}

fn files_payload(rows: &BTreeMap<String, Value>) -> Vec<Value> {
    rows.values().cloned().collect()
}

fn feature_executions_payload(rows: &BTreeMap<String, Value>) -> Vec<Value> {
    rows.iter()
        .map(|(feature_file, row)| {
            json!({
                "feature_file": feature_file,
                "status": row.get("status").and_then(Value::as_str).unwrap_or("unknown"),
                "exit_code": row.get("exit_code").and_then(Value::as_i64).unwrap_or(0),
            })
        })
        .collect()
}

fn wave_source(wave: &str) -> Option<&'static str> {
    match wave {
        "A" => Some(BEHAVIOR_SOURCE_A),
        "B" => Some(BEHAVIOR_SOURCE_B),
        "C" => Some(BEHAVIOR_SOURCE_C),
        _ => None,
    }
}

fn repo_path(path: &Path) -> String {
    let cwd = env::current_dir().ok();
    let rel = cwd
        .as_deref()
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path);
    rel.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

fn normalize_path(path: &str) -> Result<String> {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        let cwd = env::current_dir().context("resolve current directory")?;
        if let Ok(relative) = candidate.strip_prefix(cwd) {
            return Ok(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(path.replace('\\', "/"))
}

fn bullet_list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("  - {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn read_lines(path: &Path) -> Result<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    Ok(fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn load_json(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse JSON {}", path.display()))
}

fn load_json_optional(path: &Path) -> Result<Option<Value>> {
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(load_json(path)?))
}

fn load_json_or_empty(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(json!({}));
    }
    load_json(path)
}

fn number_field(object: Option<&Map<String, Value>>, key: &str) -> u64 {
    object
        .and_then(|object| object.get(key))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

fn float_field(object: Option<&Map<String, Value>>, key: &str) -> f64 {
    object
        .and_then(|object| object.get(key))
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}

fn percent(covered: u64, count: u64) -> f64 {
    if count == 0 {
        return 0.0;
    }
    (((covered as f64) / (count as f64)) * 10_000.0).round() / 100.0
}

fn now_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    let text = serde_json::to_string_pretty(value).context("serialize JSON")?;
    fs::write(path, format!("{text}\n")).with_context(|| format!("write {}", path.display()))
}
