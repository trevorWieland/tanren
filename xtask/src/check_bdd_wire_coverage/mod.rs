//! `xtask check-bdd-wire-coverage` — BDD step bodies must dispatch through
//! `*Harness` traits, not directly through `tanren_app_services::Handlers`.
//!
//! AST-walks `crates/tanren-bdd/src/**/*.rs`. For every async fn annotated
//! with cucumber's `#[given]`, `#[when]`, or `#[then]` attribute, walks the
//! function body and rejects any expression whose textual rendering names
//! a `Handlers::<method>` path or a `.handlers.<method>(...)` access.
//!
//! See `profiles/rust-cargo/testing/bdd-wire-harness.md`.

use anyhow::{Context, Result, bail};
use quote::ToTokens;
use std::fs;
use std::io::Write;
use std::path::Path;

use syn::visit::{self, Visit};
use syn::{Attribute, Expr, ImplItem, Item, ItemFn};

const FORBIDDEN_PATH_FRAGMENTS: &[&str] = &[
    "Handlers :: sign_up",
    "Handlers :: sign_in",
    "Handlers :: accept_invitation",
    "Handlers::sign_up",
    "Handlers::sign_in",
    "Handlers::accept_invitation",
];

const FORBIDDEN_METHOD_RECEIVERS: &[&str] = &[
    "handlers . sign_up",
    "handlers . sign_in",
    "handlers . accept_invitation",
    "handlers.sign_up",
    "handlers.sign_in",
    "handlers.accept_invitation",
];

pub(crate) fn run(root: &Path) -> Result<()> {
    let bdd_src = root.join("crates").join("tanren-bdd").join("src");
    if !bdd_src.exists() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-bdd-wire-coverage: 0 violations (no tanren-bdd/src present)"
        );
        return Ok(());
    }

    let mut violations: Vec<String> = Vec::new();
    for entry in walkdir::WalkDir::new(&bdd_src)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let src = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let Ok(file) = syn::parse_file(&src) else {
            continue;
        };
        scan_items(root, path, &src, &file.items, &mut violations);
    }

    if violations.is_empty() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-bdd-wire-coverage: 0 violations (BDD steps dispatch through *Harness traits)"
        );
        return Ok(());
    }
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    for v in &violations {
        let _ = writeln!(handle, "{v}");
    }
    bail!(
        "check-bdd-wire-coverage: {} violation(s); BDD steps must dispatch through *Harness traits",
        violations.len()
    );
}

fn scan_items(root: &Path, path: &Path, src: &str, items: &[Item], violations: &mut Vec<String>) {
    for item in items {
        match item {
            Item::Fn(f) => scan_fn(root, path, src, f, violations),
            Item::Mod(m) => {
                if let Some((_, items)) = &m.content {
                    scan_items(root, path, src, items, violations);
                }
            }
            Item::Impl(i) => {
                for ii in &i.items {
                    if let ImplItem::Fn(m) = ii {
                        let attrs = &m.attrs;
                        let body = &m.block;
                        scan_step(
                            root,
                            path,
                            src,
                            attrs,
                            &m.sig.ident.to_string(),
                            body,
                            violations,
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

fn scan_fn(root: &Path, path: &Path, src: &str, f: &ItemFn, violations: &mut Vec<String>) {
    scan_step(
        root,
        path,
        src,
        &f.attrs,
        &f.sig.ident.to_string(),
        &f.block,
        violations,
    );
}

fn is_step_attr(attr: &Attribute) -> bool {
    let path_text = attr.path().to_token_stream().to_string();
    let normalized = collapse_ws(&path_text);
    matches!(
        normalized.as_str(),
        "given" | "when" | "then" | "cucumber :: given" | "cucumber :: when" | "cucumber :: then"
    )
}

fn scan_step(
    root: &Path,
    path: &Path,
    src: &str,
    attrs: &[Attribute],
    fn_name: &str,
    body: &syn::Block,
    violations: &mut Vec<String>,
) {
    if !attrs.iter().any(is_step_attr) {
        return;
    }
    let mut v = ForbiddenVisitor { hits: Vec::new() };
    v.visit_block(body);
    for hit in v.hits {
        let line = locate_line(src, &hit, fn_name);
        violations.push(format!(
            "{}:{}: BDD step `{fn_name}` dispatches via `{hit}` — must use a *Harness trait (see profiles/rust-cargo/testing/bdd-wire-harness.md)",
            path.strip_prefix(root).unwrap_or(path).display(),
            line
        ));
    }
}

struct ForbiddenVisitor {
    hits: Vec<String>,
}

impl<'ast> Visit<'ast> for ForbiddenVisitor {
    fn visit_expr(&mut self, e: &'ast Expr) {
        match e {
            Expr::Path(p) => {
                let text = collapse_ws(&p.to_token_stream().to_string());
                for frag in FORBIDDEN_PATH_FRAGMENTS {
                    if text.contains(frag) && !self.hits.contains(&text) {
                        self.hits.push(text.clone());
                    }
                }
            }
            Expr::MethodCall(_) | Expr::Field(_) => {
                let text = collapse_ws(&e.to_token_stream().to_string());
                for frag in FORBIDDEN_METHOD_RECEIVERS {
                    if text.contains(frag) && !self.hits.contains(&text) {
                        self.hits.push(text.clone());
                        break;
                    }
                }
            }
            _ => {}
        }
        visit::visit_expr(self, e);
    }
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

fn locate_line(src: &str, _hit: &str, fn_name: &str) -> usize {
    let probe = format!("fn {fn_name}");
    if let Some(idx) = src.find(&probe) {
        return src[..idx].lines().count().max(1);
    }
    1
}
