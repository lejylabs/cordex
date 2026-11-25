//! Analyzer and safe mechanical migration tool for moving **Serenity**,
//! **Poise**, and **Twilight** Discord bots to `cordex`.
//!
//! The tool intentionally performs only deterministic, low-risk mechanical
//! rewrites (crate and type path updates, `GatewayIntents` -> `Intents`,
//! `twilight_*` module mappings) and marks non-trivial structural patterns
//! (`#[poise::command]`, `impl EventHandler`, `impl TypeMapKey`, Serenity
//! builders, Twilight shard/cache setup) with idempotent
//! `// TODO(cordex-migrate): ...` notes pointing to the migration guide.

use std::collections::BTreeSet;
use std::fmt::{self, Write as _};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Current `cordex` version recommended in generated `Cargo.toml` snippets.
pub const CORDEX_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Base URL of the `cordex` migration guide.
pub const GUIDE_URL: &str = "https://lejylabs.github.io/cordex/migration.html";

/// Prefix used for generated inline migration comments.
pub const TODO_PREFIX: &str = "// TODO(cordex-migrate):";

/// Source Discord library ecosystem detected in a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceEcosystem {
    /// `serenity`
    Serenity,
    /// `poise`
    Poise,
    /// `twilight-*` crates
    Twilight,
    /// `songbird` (voice audio transport)
    Songbird,
}

impl SourceEcosystem {
    /// Human-readable display name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Serenity => "Serenity",
            Self::Poise => "Poise",
            Self::Twilight => "Twilight",
            Self::Songbird => "Songbird",
        }
    }
}

impl fmt::Display for SourceEcosystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A Discord library dependency discovered in a `Cargo.toml` manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedDependency {
    /// Crate name as written in `Cargo.toml`.
    pub crate_name: String,
    /// Ecosystem the crate belongs to.
    pub ecosystem: SourceEcosystem,
    /// Path to the manifest containing this dependency.
    pub manifest_path: PathBuf,
    /// Declared feature flags (if inline or table-specified).
    pub features: Vec<String>,
}

/// Recommended `cordex` dependency configuration derived from detected crates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestedDependency {
    /// Recommended `cordex` crate version.
    pub version: String,
    /// Whether `default-features` should remain enabled.
    pub default_features: bool,
    /// Recommended opt-in feature flags.
    pub features: Vec<String>,
    /// Additional notes (for example about Songbird audio or HTTP-only usage).
    pub notes: Vec<String>,
}

impl SuggestedDependency {
    /// Formats a ready-to-paste `Cargo.toml` dependency line.
    pub fn to_toml_line(&self) -> String {
        if self.default_features && self.features.is_empty() {
            return format!("cordex = \"{}\"", self.version);
        }
        let mut parts = vec![format!("version = \"{}\"", self.version)];
        if !self.default_features {
            parts.push("default-features = false".to_owned());
        }
        if !self.features.is_empty() {
            let quoted: Vec<String> = self.features.iter().map(|f| format!("\"{f}\"")).collect();
            parts.push(format!("features = [{}]", quoted.join(", ")));
        }
        format!("cordex = {{ {} }}", parts.join(", "))
    }
}

/// Classification of a single migration finding in a Rust source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingKind {
    /// Can be safely rewritten automatically by `apply`.
    AutoRewrite,
    /// Requires manual restructuring; `apply` can add an inline TODO comment.
    ManualAction,
}

/// A single detected migration item in a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Path of the Rust file.
    pub path: PathBuf,
    /// 1-based line number.
    pub line: usize,
    /// Source library this pattern belongs to.
    pub ecosystem: SourceEcosystem,
    /// Whether the finding is rewritten automatically or needs manual work.
    pub kind: FindingKind,
    /// Short actionable explanation.
    pub message: String,
    /// Section anchor in `book/src/migration.md`.
    pub guide_anchor: &'static str,
}

/// Per-file summary produced by [`analyze`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePlan {
    /// Path of the scanned Rust file.
    pub path: PathBuf,
    /// Number of safe automatic rewrites in this file.
    pub rewrites: usize,
    /// Number of manual action markers in this file.
    pub manual_actions: usize,
    /// Detailed findings for this file.
    pub findings: Vec<Finding>,
}

/// Full report returned by [`analyze`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisReport {
    /// Root path that was scanned.
    pub root: PathBuf,
    /// Number of `Cargo.toml` files scanned.
    pub manifests_scanned: usize,
    /// Number of `.rs` files scanned.
    pub rust_files_scanned: usize,
    /// Detected Serenity, Poise, Twilight, and Songbird dependencies.
    pub dependencies: Vec<DetectedDependency>,
    /// Recommended `cordex` dependency configuration.
    pub suggestion: SuggestedDependency,
    /// Files with at least one finding.
    pub files: Vec<FilePlan>,
}

impl AnalysisReport {
    /// Total number of safe automatic rewrites across all files.
    pub fn total_rewrites(&self) -> usize {
        self.files.iter().map(|f| f.rewrites).sum()
    }

    /// Total number of manual follow-up items across all files.
    pub fn total_manual_actions(&self) -> usize {
        self.files.iter().map(|f| f.manual_actions).sum()
    }

    /// Distinct source ecosystems discovered in manifests and source files.
    pub fn ecosystems(&self) -> Vec<SourceEcosystem> {
        let mut set = BTreeSet::new();
        for dep in &self.dependencies {
            set.insert(dep.ecosystem);
        }
        for file in &self.files {
            for finding in &file.findings {
                set.insert(finding.ecosystem);
            }
        }
        set.into_iter().collect()
    }

    /// Formats a human-readable CLI report.
    pub fn format_text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "cordex-migrate analysis: {}", self.root.display());
        let _ = writeln!(
            out,
            "Scanned {} Cargo.toml file(s) and {} Rust source file(s).",
            self.manifests_scanned, self.rust_files_scanned
        );

        let ecosystems = self.ecosystems();
        if ecosystems.is_empty() {
            let _ = writeln!(
                out,
                "\nNo Serenity, Poise, or Twilight usage detected. Nothing to migrate."
            );
            return out;
        }

        let eco_names: Vec<&str> = ecosystems.iter().map(|e| e.as_str()).collect();
        let _ = writeln!(out, "Detected ecosystems: {}", eco_names.join(", "));

        if !self.dependencies.is_empty() {
            let _ = writeln!(out, "\nDetected dependencies:");
            for dep in &self.dependencies {
                let rel = display_rel(&self.root, &dep.manifest_path);
                if dep.features.is_empty() {
                    let _ = writeln!(out, "  - {} ({}) in {rel}", dep.crate_name, dep.ecosystem);
                } else {
                    let _ = writeln!(
                        out,
                        "  - {} ({}) [features: {}] in {rel}",
                        dep.crate_name,
                        dep.ecosystem,
                        dep.features.join(", ")
                    );
                }
            }
        }

        let _ = writeln!(out, "\nRecommended Cargo.toml dependency:");
        let _ = writeln!(out, "  {}", self.suggestion.to_toml_line());
        for note in &self.suggestion.notes {
            let _ = writeln!(out, "  Note: {note}");
        }

        let _ = writeln!(
            out,
            "\nSource scan summary: {} file(s) affected, {} safe auto-rewrite(s), {} manual action(s).",
            self.files.len(),
            self.total_rewrites(),
            self.total_manual_actions()
        );

        for file in &self.files {
            let rel = display_rel(&self.root, &file.path);
            let _ = writeln!(
                out,
                "\n{rel} ({} auto, {} manual):",
                file.rewrites, file.manual_actions
            );
            for finding in &file.findings {
                let tag = match finding.kind {
                    FindingKind::AutoRewrite => "AUTO  ",
                    FindingKind::ManualAction => "MANUAL",
                };
                let _ = writeln!(out, "  L{:<4} [{tag}] {}", finding.line, finding.message);
            }
        }

        let _ = writeln!(out, "\nNext steps:");
        let _ = writeln!(
            out,
            "  1. Preview mechanical rewrites: cordex-migrate apply {}",
            self.root.display()
        );
        let _ = writeln!(
            out,
            "  2. Write changes in place:      cordex-migrate apply {} --write",
            self.root.display()
        );
        let _ = writeln!(out, "  3. Follow the recipes:          {GUIDE_URL}");
        out
    }
}

/// Options controlling [`apply`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplyOptions {
    /// When `false` (default), compute diffs without modifying files on disk.
    pub write: bool,
    /// When `true` (default), insert idempotent `// TODO(cordex-migrate):` comments
    /// above non-trivial patterns that require manual restructuring.
    pub annotate_todos: bool,
}

impl Default for ApplyOptions {
    fn default() -> Self {
        Self {
            write: false,
            annotate_todos: true,
        }
    }
}

/// Summary of changes applied (or previewed) for one Rust source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// Path of the modified file.
    pub path: PathBuf,
    /// Number of safe automatic rewrites performed.
    pub rewrites: usize,
    /// Number of inline TODO comments inserted.
    pub todos_added: usize,
    /// Compact line-based diff preview.
    pub diff: String,
}

/// Full result returned by [`apply`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyReport {
    /// Root path that was processed.
    pub root: PathBuf,
    /// Whether changes were written to disk (`false` means dry-run).
    pub wrote_files: bool,
    /// Per-file changes.
    pub changes: Vec<FileChange>,
    /// Recommended `cordex` dependency configuration.
    pub suggestion: SuggestedDependency,
}

impl ApplyReport {
    /// Formats a human-readable summary and diff preview.
    pub fn format_text(&self) -> String {
        let mut out = String::new();
        let mode = if self.wrote_files {
            "APPLIED"
        } else {
            "DRY-RUN (no files modified; pass --write to apply)"
        };
        let _ = writeln!(out, "cordex-migrate apply [{mode}]");

        let total_rewrites: usize = self.changes.iter().map(|c| c.rewrites).sum();
        let total_todos: usize = self.changes.iter().map(|c| c.todos_added).sum();
        let _ = writeln!(
            out,
            "{} file(s) changed, {} auto-rewrite(s), {} TODO comment(s) added.",
            self.changes.len(),
            total_rewrites,
            total_todos
        );
        let _ = writeln!(
            out,
            "Recommended Cargo.toml entry:\n  {}",
            self.suggestion.to_toml_line()
        );

        for change in &self.changes {
            let rel = display_rel(&self.root, &change.path);
            let _ = writeln!(
                out,
                "\n--- {rel} ({} rewrite(s), {} TODO(s)) ---",
                change.rewrites, change.todos_added
            );
            out.push_str(&change.diff);
        }
        out
    }
}

/// Scans a project directory (or single file) for Serenity, Poise, and Twilight
/// usage and returns a migration plan without touching any files.
pub fn analyze(root: &Path) -> io::Result<AnalysisReport> {
    let (manifests, rust_files) = collect_project_files(root)?;
    let mut dependencies = Vec::new();
    for manifest in &manifests {
        let content = fs::read_to_string(manifest)?;
        dependencies.extend(scan_manifest(manifest, &content));
    }

    let mut files = Vec::new();
    for path in &rust_files {
        let content = fs::read_to_string(path)?;
        let transformed = transform_source(path, &content, true);
        if !transformed.findings.is_empty() {
            let manual_actions = transformed
                .findings
                .iter()
                .filter(|f| f.kind == FindingKind::ManualAction)
                .count();
            files.push(FilePlan {
                path: path.clone(),
                rewrites: transformed.rewrites,
                manual_actions,
                findings: transformed.findings,
            });
        }
    }

    let suggestion = build_suggestion(&dependencies, &files);
    Ok(AnalysisReport {
        root: root.to_path_buf(),
        manifests_scanned: manifests.len(),
        rust_files_scanned: rust_files.len(),
        dependencies,
        suggestion,
        files,
    })
}

/// Applies safe mechanical rewrites (and optional `// TODO(cordex-migrate):`
/// annotations) to Rust source files under `root`.
///
/// With `options.write == false` (the default), files are never modified on
/// disk and only the computed diff report is returned.
pub fn apply(root: &Path, options: ApplyOptions) -> io::Result<ApplyReport> {
    let (manifests, rust_files) = collect_project_files(root)?;
    let mut dependencies = Vec::new();
    for manifest in &manifests {
        let content = fs::read_to_string(manifest)?;
        dependencies.extend(scan_manifest(manifest, &content));
    }

    let mut changes = Vec::new();
    let mut plans = Vec::new();

    for path in &rust_files {
        let original = fs::read_to_string(path)?;
        let transformed = transform_source(path, &original, options.annotate_todos);
        if !transformed.findings.is_empty() {
            let manual_actions = transformed
                .findings
                .iter()
                .filter(|f| f.kind == FindingKind::ManualAction)
                .count();
            plans.push(FilePlan {
                path: path.clone(),
                rewrites: transformed.rewrites,
                manual_actions,
                findings: transformed.findings,
            });
        }

        if transformed.output != original {
            let diff = render_diff(&original, &transformed.output);
            if options.write {
                fs::write(path, &transformed.output)?;
            }
            changes.push(FileChange {
                path: path.clone(),
                rewrites: transformed.rewrites,
                todos_added: transformed.todos_added,
                diff,
            });
        }
    }

    let suggestion = build_suggestion(&dependencies, &plans);
    Ok(ApplyReport {
        root: root.to_path_buf(),
        wrote_files: options.write,
        changes,
        suggestion,
    })
}

struct RewriteRule {
    from: &'static str,
    to: &'static str,
    ecosystem: SourceEcosystem,
    note: &'static str,
    anchor: &'static str,
}

struct ManualRule {
    needle: &'static str,
    ecosystem: SourceEcosystem,
    message: &'static str,
    anchor: &'static str,
}

/// Ordered from most specific to most general so longer paths match first.
const REWRITE_RULES: &[RewriteRule] = &[
    // Twilight specific paths
    RewriteRule {
        from: "twilight_model::gateway::Intents",
        to: "cordex::model::gateway::Intents",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_model::gateway::Intents` -> `cordex::model::gateway::Intents`",
        anchor: "intents--permissions",
    },
    RewriteRule {
        from: "twilight_model::guild::Permissions",
        to: "cordex::model::Permissions",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_model::guild::Permissions` -> `cordex::model::Permissions`",
        anchor: "intents--permissions",
    },
    RewriteRule {
        from: "twilight_model::id::Id",
        to: "cordex::model::id::Id",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_model::id::Id` -> `cordex::model::id::Id`",
        anchor: "type--method-cheatsheet",
    },
    RewriteRule {
        from: "twilight_model::id::marker::",
        to: "cordex::model::id::marker::",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_model::id::marker::` -> `cordex::model::id::marker::`",
        anchor: "type--method-cheatsheet",
    },
    RewriteRule {
        from: "twilight_gateway::EventTypeFlags",
        to: "cordex::gateway::EventTypeFlags",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_gateway::EventTypeFlags` -> `cordex::gateway::EventTypeFlags`",
        anchor: "recipe-10-low-level-sharding--event-loop-twilight",
    },
    RewriteRule {
        from: "twilight_gateway::ShardId",
        to: "cordex::gateway::ShardId",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_gateway::ShardId` -> `cordex::gateway::ShardId`",
        anchor: "recipe-10-low-level-sharding--event-loop-twilight",
    },
    RewriteRule {
        from: "twilight_gateway::Shard",
        to: "cordex::gateway::Shard",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_gateway::Shard` -> `cordex::gateway::Shard`",
        anchor: "recipe-10-low-level-sharding--event-loop-twilight",
    },
    RewriteRule {
        from: "twilight_gateway::Event",
        to: "cordex::model::gateway::Event",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_gateway::Event` -> `cordex::model::gateway::Event`",
        anchor: "recipe-10-low-level-sharding--event-loop-twilight",
    },
    RewriteRule {
        from: "twilight_http::Client",
        to: "cordex::http::Client",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_http::Client` -> `cordex::http::Client`",
        anchor: "type--method-cheatsheet",
    },
    RewriteRule {
        from: "twilight_cache_inmemory::InMemoryCache",
        to: "cordex::cache::InMemoryCache",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_cache_inmemory::InMemoryCache` -> `cordex::cache::InMemoryCache`",
        anchor: "recipe-8-opt-in-cache--rest-fallback",
    },
    RewriteRule {
        from: "twilight_cache_inmemory::ResourceType",
        to: "cordex::cache::ResourceType",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_cache_inmemory::ResourceType` -> `cordex::cache::ResourceType`",
        anchor: "recipe-8-opt-in-cache--rest-fallback",
    },
    RewriteRule {
        from: "twilight_standby::Standby",
        to: "cordex::gateway::Standby",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_standby::Standby` -> `cordex::gateway::Standby`",
        anchor: "recipe-4-buttons--inline-waiting",
    },
    RewriteRule {
        from: "twilight_mention::Mention",
        to: "cordex::model::mention::Mentionable",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_mention::Mention` -> `cordex::model::mention::Mentionable`",
        anchor: "type--method-cheatsheet",
    },
    RewriteRule {
        from: "twilight_lavalink::",
        to: "cordex::lavalink::",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_lavalink::` -> `cordex::lavalink::`",
        anchor: "crate-mapping",
    },
    // Twilight broad module prefixes
    RewriteRule {
        from: "twilight_model::",
        to: "cordex::model::",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_model::` -> `cordex::model::`",
        anchor: "crate-mapping",
    },
    RewriteRule {
        from: "twilight_http::",
        to: "cordex::http::",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_http::` -> `cordex::http::`",
        anchor: "crate-mapping",
    },
    RewriteRule {
        from: "twilight_gateway::",
        to: "cordex::gateway::",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_gateway::` -> `cordex::gateway::`",
        anchor: "crate-mapping",
    },
    RewriteRule {
        from: "twilight_cache_inmemory::",
        to: "cordex::cache::",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_cache_inmemory::` -> `cordex::cache::`",
        anchor: "crate-mapping",
    },
    RewriteRule {
        from: "twilight_standby::",
        to: "cordex::gateway::",
        ecosystem: SourceEcosystem::Twilight,
        note: "Rewrite `twilight_standby::` -> `cordex::gateway::`",
        anchor: "crate-mapping",
    },
    // Serenity safe type paths
    RewriteRule {
        from: "serenity::all::GatewayIntents",
        to: "cordex::model::gateway::Intents",
        ecosystem: SourceEcosystem::Serenity,
        note: "Rewrite `serenity::all::GatewayIntents` -> `cordex::model::gateway::Intents`",
        anchor: "intents--permissions",
    },
    RewriteRule {
        from: "serenity::prelude::GatewayIntents",
        to: "cordex::model::gateway::Intents",
        ecosystem: SourceEcosystem::Serenity,
        note: "Rewrite `serenity::prelude::GatewayIntents` -> `cordex::model::gateway::Intents`",
        anchor: "intents--permissions",
    },
    RewriteRule {
        from: "serenity::model::gateway::GatewayIntents",
        to: "cordex::model::gateway::Intents",
        ecosystem: SourceEcosystem::Serenity,
        note: "Rewrite `serenity::model::gateway::GatewayIntents` -> `cordex::model::gateway::Intents`",
        anchor: "intents--permissions",
    },
    RewriteRule {
        from: "serenity::model::permissions::Permissions",
        to: "cordex::model::Permissions",
        ecosystem: SourceEcosystem::Serenity,
        note: "Rewrite `serenity::model::permissions::Permissions` -> `cordex::model::Permissions`",
        anchor: "intents--permissions",
    },
    RewriteRule {
        from: "serenity::all::Permissions",
        to: "cordex::model::Permissions",
        ecosystem: SourceEcosystem::Serenity,
        note: "Rewrite `serenity::all::Permissions` -> `cordex::model::Permissions`",
        anchor: "intents--permissions",
    },
    RewriteRule {
        from: "GatewayIntents::",
        to: "Intents::",
        ecosystem: SourceEcosystem::Serenity,
        note: "Rewrite `GatewayIntents::` -> `Intents::`",
        anchor: "intents--permissions",
    },
];

const MANUAL_RULES: &[ManualRule] = &[
    ManualRule {
        needle: "#[poise::command",
        ecosystem: SourceEcosystem::Poise,
        message: "Replace `#[poise::command]` with `Bot::command(Command::slash(...), |ctx: CommandContext| async move { ... })`",
        anchor: "recipe-2-slash-command-with-options--subcommands",
    },
    ManualRule {
        needle: "poise::Framework::builder",
        ecosystem: SourceEcosystem::Poise,
        message: "Replace `poise::Framework::builder()` with `Bot::new(token)` (auto-registers commands on READY)",
        anchor: "recipe-1-minimal-bot--graceful-shutdown",
    },
    ManualRule {
        needle: "poise::Context",
        ecosystem: SourceEcosystem::Poise,
        message: "Replace `poise::Context<'_, ...>` with `CommandContext` and read shared state via `ctx.data::<T>()`",
        anchor: "recipe-6-shared-state-typemap--botdata",
    },
    ManualRule {
        needle: "impl EventHandler for",
        ecosystem: SourceEcosystem::Serenity,
        message: "Replace `EventHandler` impl with `Bot::on_ready` / `Bot::on_message` / `Bot::on_event` closures (no #[async_trait] needed)",
        anchor: "recipe-1-minimal-bot--graceful-shutdown",
    },
    ManualRule {
        needle: "impl TypeMapKey for",
        ecosystem: SourceEcosystem::Serenity,
        message: "Remove `TypeMapKey`: attach state directly with `Bot::data(value)` and read it via `ctx.data::<T>()`",
        anchor: "recipe-6-shared-state-typemap--botdata",
    },
    ManualRule {
        needle: "Client::builder(",
        ecosystem: SourceEcosystem::Serenity,
        message: "Replace Serenity `Client::builder(token, intents)` with `Bot::new(token)` (intents are derived automatically; opt into privileged ones with `.message_content()` / `.intents(...)`)",
        anchor: "recipe-1-minimal-bot--graceful-shutdown",
    },
    ManualRule {
        needle: "CreateEmbed::new(",
        ecosystem: SourceEcosystem::Serenity,
        message: "Replace `CreateEmbed::new()` with `Embed::new()` (returns Result on Discord length limits) wrapped in `Reply::embed(embed)`",
        anchor: "type--method-cheatsheet",
    },
    ManualRule {
        needle: "CreateMessage::new(",
        ecosystem: SourceEcosystem::Serenity,
        message: "Replace `CreateMessage::new()` with `Reply::new(...)` and `ctx.send(channel_id, reply)` or `ctx.reply(reply)`",
        anchor: "type--method-cheatsheet",
    },
    ManualRule {
        needle: "CreateButton::new(",
        ecosystem: SourceEcosystem::Serenity,
        message: "Replace `CreateButton::new(...)` with `Button::primary/secondary/success/danger(id, label)` inside `ActionRow::new([...])`",
        anchor: "recipe-4-buttons--inline-waiting",
    },
    ManualRule {
        needle: "CreateModal::new(",
        ecosystem: SourceEcosystem::Serenity,
        message: "Replace `CreateModal::new(...)` with `Modal::new(id, title)` and `ctx.modal(modal).await`",
        anchor: "recipe-5-modals",
    },
    ManualRule {
        needle: "serenity::all::",
        ecosystem: SourceEcosystem::Serenity,
        message: "Replace `serenity::all::*` import with `use cordex::prelude::*;`",
        anchor: "type--method-cheatsheet",
    },
    ManualRule {
        needle: "serenity::prelude::",
        ecosystem: SourceEcosystem::Serenity,
        message: "Replace `serenity::prelude::*` import with `use cordex::prelude::*;`",
        anchor: "type--method-cheatsheet",
    },
    ManualRule {
        needle: "stream::create_recommended",
        ecosystem: SourceEcosystem::Twilight,
        message: "Replace `create_recommended` with `ShardManager::builder(&token, intents).build().await?` (or `Bot::new(token)`)",
        anchor: "recipe-10-low-level-sharding--event-loop-twilight",
    },
    ManualRule {
        needle: "InMemoryCache::builder(",
        ecosystem: SourceEcosystem::Twilight,
        message: "Replace `InMemoryCache::builder()` with `CacheConfig::new().resources(...).parts(...)` and `InMemoryCache::new(config)`",
        anchor: "recipe-8-opt-in-cache--rest-fallback",
    },
    ManualRule {
        needle: "twilight_util::",
        ecosystem: SourceEcosystem::Twilight,
        message: "Helpers from `twilight-util` live directly in `cordex::model` (builders, `permission_calc`, `cdn`, `parse_message_link`, `parse_webhook_url`, `parse_invite_url`) and `cordex::interactions`",
        anchor: "crate-mapping",
    },
    ManualRule {
        needle: "songbird::",
        ecosystem: SourceEcosystem::Songbird,
        message: "cordex provides pure-Rust voice signaling (`voice` feature) and Lavalink v4 (`lavalink` feature), without C/FFI Opus/UDP audio transport",
        anchor: "crate-mapping",
    },
];

struct TransformResult {
    output: String,
    rewrites: usize,
    todos_added: usize,
    findings: Vec<Finding>,
}

fn transform_source(path: &Path, input: &str, annotate_todos: bool) -> TransformResult {
    let line_ending = if input.contains("\r\n") { "\r\n" } else { "\n" };
    let trailing_newline = input.ends_with('\n');
    let raw_lines: Vec<&str> = input.lines().collect();

    let mut out_lines: Vec<String> = Vec::with_capacity(raw_lines.len());
    let mut findings = Vec::new();
    let mut rewrites = 0usize;
    let mut todos_added = 0usize;
    let mut in_block_comment = false;

    for (idx, raw_line) in raw_lines.iter().enumerate() {
        let line_no = idx + 1;
        let line_str = *raw_line;

        if in_block_comment {
            if line_str.contains("*/") {
                in_block_comment = false;
            }
            out_lines.push(line_str.to_owned());
            continue;
        }

        let trimmed = line_str.trim_start();
        if trimmed.starts_with("//") {
            out_lines.push(line_str.to_owned());
            continue;
        }
        if trimmed.starts_with("/*") {
            if !trimmed.contains("*/") {
                in_block_comment = true;
            }
            out_lines.push(line_str.to_owned());
            continue;
        }

        let (code_part, comment_part) = split_code_and_line_comment(line_str);
        let mut updated_code = code_part.to_owned();

        // 1. Safe mechanical rewrites (code segments outside string literals only).
        for rule in REWRITE_RULES {
            let (next_code, count) = replace_outside_strings(&updated_code, rule.from, rule.to);
            if count > 0 {
                updated_code = next_code;
                rewrites += count;
                findings.push(Finding {
                    path: path.to_path_buf(),
                    line: line_no,
                    ecosystem: rule.ecosystem,
                    kind: FindingKind::AutoRewrite,
                    message: rule.note.to_owned(),
                    guide_anchor: rule.anchor,
                });
            }
        }

        // 2. Manual follow-up detection on `updated_code` so lines that were
        //    already cleanly auto-rewritten (e.g. `serenity::all::GatewayIntents`)
        //    do not emit spurious TODO comments.
        let indent_len = line_str.len() - line_str.trim_start().len();
        let indent = &line_str[..indent_len];

        for rule in MANUAL_RULES {
            if contains_outside_strings(&updated_code, rule.needle) {
                findings.push(Finding {
                    path: path.to_path_buf(),
                    line: line_no,
                    ecosystem: rule.ecosystem,
                    kind: FindingKind::ManualAction,
                    message: rule.message.to_owned(),
                    guide_anchor: rule.anchor,
                });

                if annotate_todos {
                    let todo_line = format!(
                        "{indent}{TODO_PREFIX} {} ({GUIDE_URL}#{})",
                        rule.message, rule.anchor
                    );
                    let already_annotated = out_lines
                        .last()
                        .is_some_and(|prev| prev.trim() == todo_line.trim());
                    if !already_annotated {
                        out_lines.push(todo_line);
                        todos_added += 1;
                    }
                }
            }
        }

        if comment_part.is_empty() {
            out_lines.push(updated_code);
        } else {
            let mut combined = String::with_capacity(updated_code.len() + comment_part.len());
            combined.push_str(&updated_code);
            combined.push_str(comment_part);
            out_lines.push(combined);
        }
    }

    let mut output = out_lines.join(line_ending);
    if trailing_newline {
        output.push_str(line_ending);
    }

    TransformResult {
        output,
        rewrites,
        todos_added,
        findings,
    }
}

/// Splits a line into `(code, trailing_line_comment)` without breaking on `//`
/// inside double-quoted string literals.
fn split_code_and_line_comment(line: &str) -> (&str, &str) {
    let bytes = line.as_bytes();
    let mut i = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if b == b'"' {
            in_string = true;
            i += 1;
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            return (&line[..i], &line[i..]);
        }
        i += 1;
    }

    (line, "")
}

/// Checks whether `needle` appears in `code` outside double-quoted string literals.
fn contains_outside_strings(code: &str, needle: &str) -> bool {
    let mut found = false;
    for (segment, is_string) in split_string_segments(code) {
        if !is_string && segment.contains(needle) {
            found = true;
            break;
        }
    }
    found
}

/// Replaces occurrences of `from` with `to` only outside double-quoted strings.
fn replace_outside_strings(code: &str, from: &str, to: &str) -> (String, usize) {
    if !code.contains(from) {
        return (code.to_owned(), 0);
    }

    let mut out = String::with_capacity(code.len());
    let mut count = 0usize;

    for (segment, is_string) in split_string_segments(code) {
        if is_string {
            out.push_str(segment);
        } else {
            let mut rest = segment;
            while let Some(pos) = rest.find(from) {
                out.push_str(&rest[..pos]);
                out.push_str(to);
                rest = &rest[pos + from.len()..];
                count += 1;
            }
            out.push_str(rest);
        }
    }

    (out, count)
}

/// Splits a code snippet into alternating `(slice, is_string_literal)` parts.
fn split_string_segments(code: &str) -> Vec<(&str, bool)> {
    let bytes = code.as_bytes();
    let mut segments = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                // Include the closing quote in the string segment.
                segments.push((&code[start..=i], true));
                start = i + 1;
                in_string = false;
            }
            i += 1;
        } else if b == b'"' {
            if i > start {
                segments.push((&code[start..i], false));
            }
            start = i;
            in_string = true;
            i += 1;
        } else {
            i += 1;
        }
    }

    if start < code.len() {
        segments.push((&code[start..], in_string));
    }

    segments
}

fn collect_project_files(root: &Path) -> io::Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    let mut manifests = Vec::new();
    let mut rust_files = Vec::new();

    if root.is_file() {
        match root.file_name().and_then(|n| n.to_str()) {
            Some("Cargo.toml") => manifests.push(root.to_path_buf()),
            _ if root.extension().and_then(|e| e.to_str()) == Some("rs") => {
                rust_files.push(root.to_path_buf());
            }
            _ => {}
        }
        return Ok((manifests, rust_files));
    }

    walk_dir(root, &mut manifests, &mut rust_files)?;
    manifests.sort();
    rust_files.sort();
    Ok((manifests, rust_files))
}

fn walk_dir(
    dir: &Path,
    manifests: &mut Vec<PathBuf>,
    rust_files: &mut Vec<PathBuf>,
) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_symlink() {
            continue;
        }

        if file_type.is_dir() {
            let name = entry.file_name();
            let skip = matches!(
                name.to_str(),
                Some("target" | ".git" | "node_modules" | ".cargo" | "vendor")
            );
            if !skip {
                walk_dir(&path, manifests, rust_files)?;
            }
            continue;
        }

        if file_type.is_file() {
            if path.file_name().and_then(|n| n.to_str()) == Some("Cargo.toml") {
                manifests.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                rust_files.push(path);
            }
        }
    }
    Ok(())
}

fn scan_manifest(manifest_path: &Path, content: &str) -> Vec<DetectedDependency> {
    let mut detected = Vec::new();
    let mut in_deps = false;
    let mut active_table_dep: Option<(String, SourceEcosystem)> = None;

    for raw_line in content.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            let header = &line[1..line.len() - 1].trim();
            active_table_dep = None;

            if is_deps_section(header) {
                in_deps = true;
                continue;
            }

            if let Some(dep_name) = header
                .strip_prefix("dependencies.")
                .or_else(|| header.strip_prefix("workspace.dependencies."))
                .or_else(|| header.strip_prefix("dev-dependencies."))
                && let Some(eco) = classify_crate(dep_name)
            {
                in_deps = false;
                detected.push(DetectedDependency {
                    crate_name: dep_name.to_owned(),
                    ecosystem: eco,
                    manifest_path: manifest_path.to_path_buf(),
                    features: Vec::new(),
                });
                active_table_dep = Some((dep_name.to_owned(), eco));
                continue;
            }

            in_deps = false;
            continue;
        }

        if let Some((ref name, _)) = active_table_dep {
            if let Some(rest) = line.strip_prefix("features")
                && let Some(rhs) = rest.trim_start().strip_prefix('=')
                && let Some(last) = detected.iter_mut().rev().find(|d| d.crate_name == *name)
            {
                last.features.extend(parse_feature_list(rhs));
            }
            continue;
        }

        if !in_deps {
            continue;
        }

        let Some((lhs, rhs)) = line.split_once('=') else {
            continue;
        };
        let key = lhs.trim().trim_matches('"');
        if let Some(eco) = classify_crate(key) {
            let features = extract_inline_features(rhs);
            detected.push(DetectedDependency {
                crate_name: key.to_owned(),
                ecosystem: eco,
                manifest_path: manifest_path.to_path_buf(),
                features,
            });
        }
    }

    detected
}

fn is_deps_section(header: &str) -> bool {
    matches!(
        header,
        "dependencies" | "dev-dependencies" | "workspace.dependencies"
    )
}

fn classify_crate(name: &str) -> Option<SourceEcosystem> {
    if name == "serenity" {
        Some(SourceEcosystem::Serenity)
    } else if name == "poise" {
        Some(SourceEcosystem::Poise)
    } else if name == "songbird" {
        Some(SourceEcosystem::Songbird)
    } else if name.starts_with("twilight-") {
        Some(SourceEcosystem::Twilight)
    } else {
        None
    }
}

fn extract_inline_features(rhs: &str) -> Vec<String> {
    let Some(pos) = rhs.find("features") else {
        return Vec::new();
    };
    let after = &rhs[pos + "features".len()..];
    let Some((_, list_part)) = after.split_once('=') else {
        return Vec::new();
    };
    parse_feature_list(list_part)
}

fn parse_feature_list(input: &str) -> Vec<String> {
    let Some(open) = input.find('[') else {
        return Vec::new();
    };
    let rest = &input[open + 1..];
    let slice = rest.find(']').map_or(rest, |close| &rest[..close]);
    slice
        .split(',')
        .map(str::trim)
        .filter_map(|item| {
            let trimmed = item.trim_matches('"').trim_matches('\'');
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        })
        .collect()
}

fn build_suggestion(
    dependencies: &[DetectedDependency],
    files: &[FilePlan],
) -> SuggestedDependency {
    let mut features = BTreeSet::new();
    let mut notes = Vec::new();

    let mut has_gateway = false;
    let mut has_http = false;
    let mut has_serenity_or_poise = false;

    for dep in dependencies {
        match dep.crate_name.as_str() {
            "serenity" => {
                has_serenity_or_poise = true;
                has_gateway = true;
                has_http = true;
                if dep
                    .features
                    .iter()
                    .any(|f| f == "cache" || f == "full" || f == "standard_framework")
                {
                    features.insert("cache");
                }
                if dep.features.iter().any(|f| f == "voice") {
                    features.insert("voice");
                }
            }
            "poise" => {
                has_serenity_or_poise = true;
                has_gateway = true;
                has_http = true;
            }
            "songbird" => {
                features.insert("voice");
                notes.push(
                    "Detected `songbird`: cordex includes pure-Rust voice signaling (`voice`) and Lavalink v4 (`lavalink`), but not C/FFI Opus/UDP audio transport.".to_owned(),
                );
            }
            "twilight-cache-inmemory" => {
                features.insert("cache");
            }
            "twilight-lavalink" => {
                features.insert("lavalink");
            }
            "twilight-gateway" | "twilight-standby" | "twilight-gateway-queue" => {
                has_gateway = true;
            }
            "twilight-http" => {
                has_http = true;
            }
            _ => {}
        }
    }

    // Also infer cache usage if source files reference `InMemoryCache`.
    for file in files {
        for finding in &file.findings {
            if finding.message.contains("InMemoryCache") {
                features.insert("cache");
            }
        }
    }

    let default_features = if !has_serenity_or_poise && has_http && !has_gateway {
        features.insert("http");
        notes.push(
            "Only `twilight-http` was detected: disabled default features to skip the gateway crate.".to_owned(),
        );
        false
    } else {
        true
    };

    if features.contains("lavalink") {
        // `lavalink` already enables `voice`.
        features.remove("voice");
    }

    SuggestedDependency {
        version: CORDEX_VERSION.to_owned(),
        default_features,
        features: features.into_iter().map(str::to_owned).collect(),
        notes,
    }
}

fn render_diff(original: &str, updated: &str) -> String {
    let old_lines: Vec<&str> = original.lines().collect();
    let new_lines: Vec<&str> = updated.lines().collect();
    let mut out = String::new();

    let mut i = 0usize;
    let mut j = 0usize;

    while i < old_lines.len() || j < new_lines.len() {
        if i < old_lines.len() && j < new_lines.len() && old_lines[i] == new_lines[j] {
            i += 1;
            j += 1;
            continue;
        }

        // Handle inserted TODO comment lines cleanly so diffs stay readable.
        if j < new_lines.len() && new_lines[j].trim_start().starts_with(TODO_PREFIX) {
            let _ = writeln!(out, "+ {}", new_lines[j]);
            j += 1;
            continue;
        }

        if i < old_lines.len() {
            let _ = writeln!(out, "- {}", old_lines[i]);
            i += 1;
        }
        if j < new_lines.len() {
            let _ = writeln!(out, "+ {}", new_lines[j]);
            j += 1;
        }
    }

    out
}

fn display_rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempProject {
        dir: PathBuf,
    }

    impl TempProject {
        fn new(label: &str) -> io::Result<Self> {
            let id = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "cordex-migrate-test-{label}-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(dir.join("src"))?;
            Ok(Self { dir })
        }

        fn write(&self, rel: &str, content: &str) -> io::Result<PathBuf> {
            let path = self.dir.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, content)?;
            Ok(path)
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn analyzes_serenity_and_poise_project() -> io::Result<()> {
        let project = TempProject::new("serenity-poise")?;
        project.write(
            "Cargo.toml",
            r#"[package]
name = "demo-bot"
version = "0.1.0"
edition = "2021"

[dependencies]
serenity = { version = "0.12", features = ["cache", "gateway", "rustls_backend"] }
poise = "0.6"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
"#,
        )?;
        project.write(
            "src/main.rs",
            r"use serenity::all::GatewayIntents;

struct Data;

#[poise::command(slash_command)]
async fn ping(ctx: poise::Context<'_, Data, ()>) -> Result<(), ()> {
    let _ = GatewayIntents::GUILD_MESSAGES;
    Ok(())
}
",
        )?;

        let report = analyze(&project.dir)?;
        assert_eq!(report.manifests_scanned, 1);
        assert_eq!(report.rust_files_scanned, 1);
        assert_eq!(report.dependencies.len(), 2);
        assert!(report.suggestion.default_features);
        assert_eq!(report.suggestion.features, vec!["cache".to_owned()]);
        assert_eq!(report.total_rewrites(), 2);
        assert_eq!(report.total_manual_actions(), 2);
        assert!(report.format_text().contains("Serenity, Poise"));
        Ok(())
    }

    #[test]
    fn applies_twilight_rewrites_and_is_idempotent() -> io::Result<()> {
        let project = TempProject::new("twilight")?;
        project.write(
            "Cargo.toml",
            r#"[package]
name = "tw-bot"
version = "0.1.0"

[dependencies]
twilight-model = "0.16"
twilight-http = "0.16"
twilight-gateway = "0.16"
twilight-cache-inmemory = "0.16"
twilight-standby = "0.16"
twilight-lavalink = "0.16"
"#,
        )?;
        let main_rs = project.write(
            "src/main.rs",
            r#"use twilight_model::gateway::Intents;
use twilight_model::id::Id;
use twilight_http::Client;
use twilight_cache_inmemory::InMemoryCache;
use twilight_standby::Standby;

fn setup() {
    // twilight_http::Client inside a comment must stay untouched
    let label = "twilight_model::gateway::Intents";
    let _ = label;
    let _cache = InMemoryCache::builder().build();
}
"#,
        )?;

        // Dry-run must not modify disk.
        let before = fs::read_to_string(&main_rs)?;
        let dry = apply(
            &project.dir,
            ApplyOptions {
                write: false,
                annotate_todos: true,
            },
        )?;
        assert!(!dry.wrote_files);
        assert_eq!(dry.changes.len(), 1);
        assert_eq!(fs::read_to_string(&main_rs)?, before);
        assert_eq!(
            dry.suggestion.features,
            vec!["cache".to_owned(), "lavalink".to_owned()]
        );

        // Apply with write=true.
        let written = apply(
            &project.dir,
            ApplyOptions {
                write: true,
                annotate_todos: true,
            },
        )?;
        assert!(written.wrote_files);
        assert_eq!(written.changes.len(), 1);

        let after = fs::read_to_string(&main_rs)?;
        assert!(after.contains("use cordex::model::gateway::Intents;"));
        assert!(after.contains("use cordex::model::id::Id;"));
        assert!(after.contains("use cordex::http::Client;"));
        assert!(after.contains("use cordex::cache::InMemoryCache;"));
        assert!(after.contains("use cordex::gateway::Standby;"));
        // Comments and string literals must remain untouched.
        assert!(after.contains("// twilight_http::Client inside a comment must stay untouched"));
        assert!(after.contains("let label = \"twilight_model::gateway::Intents\";"));
        assert!(after.contains(TODO_PREFIX));

        // Second run must be a no-op (idempotent).
        let second = apply(
            &project.dir,
            ApplyOptions {
                write: true,
                annotate_todos: true,
            },
        )?;
        assert_eq!(second.changes, Vec::<FileChange>::new());
        assert_eq!(fs::read_to_string(&main_rs)?, after);
        Ok(())
    }

    #[test]
    fn suggests_http_only_when_only_twilight_http_is_present() -> io::Result<()> {
        let project = TempProject::new("http-only")?;
        project.write(
            "Cargo.toml",
            r#"[package]
name = "rest-worker"
version = "0.1.0"

[dependencies]
twilight-http = "0.16"
"#,
        )?;

        let report = analyze(&project.dir)?;
        assert!(!report.suggestion.default_features);
        assert_eq!(report.suggestion.features, vec!["http".to_owned()]);
        assert!(
            report
                .suggestion
                .to_toml_line()
                .contains("default-features = false")
        );
        Ok(())
    }
}
