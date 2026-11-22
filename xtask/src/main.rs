//! Project tasks (PLAN.md §18.1).
//!
//! - `cargo xtask check`: fmt check, clippy, tests, docs, cargo-deny, layout-check
//! - `cargo xtask layout-check`: crate dependency rules (PLAN.md §4.2)

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Allowed internal dependencies per crate folder in `crates/`, as folder
/// names (PLAN.md §4.2). Package names are read from the manifests, so the
/// project name only lives in the `Cargo.toml` files.
const INTERNAL_DEPS: &[(&str, &[&str])] = &[
    ("model", &[]),
    ("http", &["model"]),
    ("gateway", &["model", "http"]),
    ("cache", &["model"]),
    ("interactions", &["model", "http"]),
    (
        "facade",
        &["model", "http", "gateway", "cache", "interactions"],
    ),
];

/// `cordex-model` stays free of async and network code.
const MODEL_FORBIDDEN: &[&str] = &[
    "tokio",
    "tokio-util",
    "tokio-websockets",
    "hyper",
    "hyper-util",
    "hyper-rustls",
    "rustls",
    "reqwest",
];

/// Other Discord libraries must never appear in the dependency graph.
const BANNED_PACKAGES: &[&str] = &[
    "twilight-model",
    "twilight-http",
    "twilight-gateway",
    "serenity",
    "poise",
    "songbird",
    "discord",
];

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let result = match task.as_str() {
        "check" => check(),
        "layout-check" => layout_check(),
        _ => Err("usage: cargo xtask <check|layout-check>".to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn run(args: &[&str], env: &[(&str, &str)]) -> Result<(), String> {
    println!("> cargo {}", args.join(" "));
    let status = Command::new("cargo")
        .args(args)
        .envs(env.iter().copied())
        .current_dir(workspace_root())
        .status()
        .map_err(|e| format!("cargo {}: {e}", args.join(" ")))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo {} failed", args.join(" ")))
    }
}

fn check() -> Result<(), String> {
    run(&["fmt", "--all", "--check"], &[])?;
    run(
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--locked",
            "--",
            "-D",
            "warnings",
        ],
        &[],
    )?;
    run(&["test", "--workspace", "--all-features", "--locked"], &[])?;
    run(
        &[
            "doc",
            "--workspace",
            "--no-deps",
            "--all-features",
            "--locked",
        ],
        &[("RUSTDOCFLAGS", "-D warnings")],
    )?;
    run(&["deny", "--locked", "check"], &[])?;
    layout_check()
}

fn layout_check() -> Result<(), String> {
    let root = workspace_root();
    let mut errors = Vec::new();

    // Package name of every crate folder.
    let mut packages = Vec::new();
    for (folder, _) in INTERNAL_DEPS {
        let manifest = root.join("crates").join(folder).join("Cargo.toml");
        let name = package_name(&read(&manifest)?)
            .ok_or_else(|| format!("{}: no package name", manifest.display()))?;
        packages.push((*folder, name));
    }
    let folder_of = |package: &str| packages.iter().find(|(_, p)| p == package).map(|(f, _)| *f);

    for (folder, allowed) in INTERNAL_DEPS {
        let manifest = root.join("crates").join(folder).join("Cargo.toml");
        for dep in dependency_names(&read(&manifest)?) {
            if let Some(dep_folder) = folder_of(&dep)
                && !allowed.contains(&dep_folder)
            {
                errors.push(format!(
                    "{}: crates/{folder} must not depend on crates/{dep_folder}",
                    manifest.display()
                ));
            }
            if *folder == "model" && MODEL_FORBIDDEN.contains(&dep.as_str()) {
                errors.push(format!(
                    "{}: crates/model must not depend on `{dep}` (no async/network code)",
                    manifest.display()
                ));
            }
        }
    }

    for dir in fs::read_dir(root.join("crates")).map_err(|e| format!("crates: {e}"))? {
        let name = dir
            .map_err(|e| e.to_string())?
            .file_name()
            .to_string_lossy()
            .into_owned();
        if !INTERNAL_DEPS.iter().any(|(k, _)| *k == name) {
            errors.push(format!(
                "crates/{name}: folder missing in INTERNAL_DEPS of xtask"
            ));
        }
    }

    let lock = read(&root.join("Cargo.lock"))?;
    for line in lock.lines() {
        if let Some(name) = line
            .strip_prefix("name = \"")
            .and_then(|r| r.strip_suffix('"'))
            && BANNED_PACKAGES.contains(&name)
        {
            errors.push(format!(
                "Cargo.lock: banned Discord library `{name}` in the dependency graph"
            ));
        }
    }

    if errors.is_empty() {
        println!("layout-check: {} crates ok", INTERNAL_DEPS.len());
        Ok(())
    } else {
        Err(format!("layout-check failed:\n{}", errors.join("\n")))
    }
}

/// Names in `[dependencies]` and `[build-dependencies]` of a manifest
/// (`name = ...` or `name.workspace = true`).
fn dependency_names(manifest: &str) -> Vec<String> {
    let mut in_deps = false;
    let mut out = Vec::new();
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_deps = line == "[dependencies]" || line == "[build-dependencies]";
            continue;
        }
        if !in_deps || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let name: String = line
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if !name.is_empty() {
            out.push(name);
        }
    }
    out
}

/// `name = "..."` in the `[package]` section of a manifest.
fn package_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_package = line == "[package]";
        } else if in_package
            && let Some(rest) = line.strip_prefix("name")
            && let Some(value) = rest.trim_start().strip_prefix('=')
        {
            return Some(value.trim().trim_matches('"').to_owned());
        }
    }
    None
}

fn read(file: &Path) -> Result<String, String> {
    fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dependency_names() {
        let manifest = "[package]\nname = \"x\"\n\n[dependencies]\n\
                        cordex-model.workspace = true\nserde = { version = \"1\" }\n\
                        # comment\n\n[features]\ncache = []\n";
        assert_eq!(dependency_names(manifest), vec!["cordex-model", "serde"]);
        assert_eq!(package_name(manifest).as_deref(), Some("x"));
    }
}
