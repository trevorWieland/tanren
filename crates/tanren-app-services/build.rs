use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crate is under crates/");
    let commands = collect_markdown(repo_root, "commands");
    let profiles = collect_markdown(repo_root, "profiles");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let mut out =
        fs::File::create(out_dir.join("embedded_assets.rs")).expect("create embedded_assets.rs");

    write_assets(&mut out, "COMMAND_ASSETS", &commands);
    write_assets(&mut out, "PROFILE_ASSETS", &profiles);
}

fn collect_markdown(repo_root: &Path, rel_root: &str) -> Vec<String> {
    let root = repo_root.join(rel_root);
    println!("cargo:rerun-if-changed={}", root.display());

    let mut out = Vec::new();
    collect_markdown_inner(repo_root, &root, &mut out);
    out.sort();
    out
}

fn collect_markdown_inner(repo_root: &Path, dir: &Path, out: &mut Vec<String>) {
    for entry in fs::read_dir(dir).expect("read asset directory") {
        let entry = entry.expect("read asset entry");
        let path = entry.path();
        if path.is_dir() {
            collect_markdown_inner(repo_root, &path, out);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        println!("cargo:rerun-if-changed={}", path.display());
        let rel = path
            .strip_prefix(repo_root)
            .expect("asset below repo root")
            .to_string_lossy()
            .replace('\\', "/");
        out.push(rel);
    }
}

fn write_assets(out: &mut fs::File, const_name: &str, paths: &[String]) {
    writeln!(out, "pub const {const_name}: &[EmbeddedAsset] = &[").expect("write asset const");
    for path in paths {
        writeln!(
            out,
            "    EmbeddedAsset {{ path: {path:?}, contents: include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../{path}\")) }},"
        )
        .expect("write asset entry");
    }
    writeln!(out, "];").expect("close asset const");
}
