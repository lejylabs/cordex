//! `cargo xtask coverage`: compare the library with the official API docs.
//!
//! The docs come from `discord/discord-api-docs` at the commit pinned in
//! `docs/discord-api-docs.rev` (checked out into `target/discord-api-docs`
//! with `git`). Measured today:
//!
//! - REST endpoints (`<Route method="…">` in the docs, the
//!   `` `METHOD /path` `` doc comments in `cordex-http`),
//! - gateway receive events, gateway opcodes, gateway close codes,
//! - component types and the value tables of the model enums in
//!   [`ENUMS`].
//!
//! Object fields and bitflags are not measured yet; the report says so.
//!
//! Flags: `--check` fails if `docs/coverage.md` is out of date,
//! `--rev <sha|latest>` measures another docs commit and writes the report
//! to `target/coverage-<rev>.md` instead (used by the docs watch workflow),
//! `--pin` moves the pin to the measured commit.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{read, workspace_root};

const DOCS_REPO: &str = "https://github.com/discord/discord-api-docs";
const REV_FILE: &str = "docs/discord-api-docs.rev";
const REPORT: &str = "docs/coverage.md";

/// Model enums whose values are compared with a docs table:
/// (file in `crates/model/src`, enum, docs page in `developers/`, heading).
const ENUMS: &[(&str, &str, &str, &str)] = &[
    (
        "interaction.rs",
        "InteractionType",
        "interactions/receiving-and-responding.mdx",
        "Interaction Type",
    ),
    (
        "interaction.rs",
        "InteractionCallbackType",
        "interactions/receiving-and-responding.mdx",
        "Interaction Callback Type",
    ),
    (
        "command.rs",
        "CommandType",
        "interactions/application-commands.mdx",
        "Application Command Types",
    ),
    (
        "command.rs",
        "CommandOptionType",
        "interactions/application-commands.mdx",
        "Application Command Option Type",
    ),
    (
        "command.rs",
        "CommandPermissionType",
        "interactions/application-commands.mdx",
        "Application Command Permission Type",
    ),
    (
        "command.rs",
        "InteractionContextType",
        "interactions/receiving-and-responding.mdx",
        "Interaction Context Types",
    ),
    (
        "command.rs",
        "EntryPointHandlerType",
        "interactions/application-commands.mdx",
        "Entry Point Command Handler Types",
    ),
    (
        "component.rs",
        "ButtonStyle",
        "components/reference.mdx",
        "Button Styles",
    ),
    (
        "component.rs",
        "TextInputStyle",
        "components/reference.mdx",
        "Text Input Styles",
    ),
    (
        "channel.rs",
        "ChannelType",
        "resources/channel.mdx",
        "Channel Types",
    ),
    (
        "channel.rs",
        "VideoQualityMode",
        "resources/channel.mdx",
        "Video Quality Modes",
    ),
    (
        "channel.rs",
        "SortOrderType",
        "resources/channel.mdx",
        "Sort Order Types",
    ),
    (
        "channel.rs",
        "ForumLayoutType",
        "resources/channel.mdx",
        "Forum Layout Types",
    ),
    (
        "message/mod.rs",
        "MessageType",
        "resources/message.mdx",
        "Message Types",
    ),
    (
        "message/mod.rs",
        "MessageActivityType",
        "resources/message.mdx",
        "Message Activity Types",
    ),
    (
        "message/mod.rs",
        "MessageReferenceType",
        "resources/message.mdx",
        "Message Reference Types",
    ),
    (
        "message/mod.rs",
        "ReactionType",
        "resources/message.mdx",
        "Reaction Types",
    ),
    (
        "audit_log.rs",
        "AuditLogEvent",
        "resources/audit-log.mdx",
        "Audit Log Events",
    ),
    (
        "auto_moderation.rs",
        "AutoModerationTriggerType",
        "resources/auto-moderation.mdx",
        "Trigger Types",
    ),
    (
        "auto_moderation.rs",
        "KeywordPresetType",
        "resources/auto-moderation.mdx",
        "Keyword Preset Types",
    ),
    (
        "auto_moderation.rs",
        "AutoModerationEventType",
        "resources/auto-moderation.mdx",
        "Event Types",
    ),
    (
        "auto_moderation.rs",
        "AutoModerationActionType",
        "resources/auto-moderation.mdx",
        "Action Types",
    ),
    (
        "guild/mod.rs",
        "DefaultMessageNotificationLevel",
        "resources/guild.mdx",
        "Default Message Notification Level",
    ),
    (
        "guild/mod.rs",
        "ExplicitContentFilterLevel",
        "resources/guild.mdx",
        "Explicit Content Filter Level",
    ),
    (
        "guild/mod.rs",
        "MfaLevel",
        "resources/guild.mdx",
        "MFA Level",
    ),
    (
        "guild/mod.rs",
        "VerificationLevel",
        "resources/guild.mdx",
        "Verification Level",
    ),
    (
        "guild/mod.rs",
        "NsfwLevel",
        "resources/guild.mdx",
        "Guild Age-Restriction Level",
    ),
    (
        "guild/mod.rs",
        "PremiumTier",
        "resources/guild.mdx",
        "Premium Tier",
    ),
    (
        "guild/integration.rs",
        "IntegrationExpireBehavior",
        "resources/guild.mdx",
        "Integration Expire Behaviors",
    ),
    (
        "invite.rs",
        "InviteType",
        "resources/invite.mdx",
        "Invite Types",
    ),
    (
        "invite.rs",
        "InviteTargetType",
        "resources/invite.mdx",
        "Invite Target Types",
    ),
    (
        "scheduled_event.rs",
        "ScheduledEventPrivacyLevel",
        "resources/guild-scheduled-event.mdx",
        "Guild Scheduled Event Privacy Level",
    ),
    (
        "scheduled_event.rs",
        "ScheduledEventEntityType",
        "resources/guild-scheduled-event.mdx",
        "Guild Scheduled Event Entity Types",
    ),
    (
        "scheduled_event.rs",
        "ScheduledEventStatus",
        "resources/guild-scheduled-event.mdx",
        "Guild Scheduled Event Status",
    ),
    (
        "scheduled_event.rs",
        "RecurrenceFrequency",
        "resources/guild-scheduled-event.mdx",
        "Guild Scheduled Event Recurrence Rule - Frequency",
    ),
    (
        "scheduled_event.rs",
        "RecurrenceWeekday",
        "resources/guild-scheduled-event.mdx",
        "Guild Scheduled Event Recurrence Rule - Weekday",
    ),
    (
        "scheduled_event.rs",
        "RecurrenceMonth",
        "resources/guild-scheduled-event.mdx",
        "Guild Scheduled Event Recurrence Rule - Month",
    ),
    (
        "sticker.rs",
        "StickerType",
        "resources/sticker.mdx",
        "Sticker Types",
    ),
    (
        "sticker.rs",
        "StickerFormatType",
        "resources/sticker.mdx",
        "Sticker Format Types",
    ),
    (
        "stage.rs",
        "StagePrivacyLevel",
        "resources/stage-instance.mdx",
        "Privacy Level",
    ),
    (
        "user.rs",
        "PremiumType",
        "resources/user.mdx",
        "Premium Types",
    ),
    (
        "webhook.rs",
        "WebhookType",
        "resources/webhook.mdx",
        "Webhook Types",
    ),
    (
        "entitlement.rs",
        "EntitlementType",
        "resources/entitlement.mdx",
        "Entitlement Types",
    ),
    (
        "application.rs",
        "ApplicationIntegrationType",
        "resources/application.mdx",
        "Application Integration Types",
    ),
    (
        "application.rs",
        "EventWebhookStatus",
        "resources/application.mdx",
        "Application Event Webhook Status",
    ),
    (
        "application.rs",
        "MembershipState",
        "topics/teams.mdx",
        "Membership State Enum",
    ),
    (
        "gateway/presence.rs",
        "ActivityType",
        "events/gateway-events.mdx",
        "Activity Types",
    ),
    (
        "gateway/presence.rs",
        "StatusDisplayType",
        "events/gateway-events.mdx",
        "Status Display Types",
    ),
    (
        "poll.rs",
        "PollLayoutType",
        "resources/poll.mdx",
        "Layout Type",
    ),
];

struct Item {
    covered: bool,
    name: String,
    detail: String,
}

struct Area {
    title: &'static str,
    items: Vec<Item>,
}

pub(crate) fn coverage(args: &[String]) -> Result<(), String> {
    let root = workspace_root();
    let check = args.iter().any(|a| a == "--check");
    let pin = args.iter().any(|a| a == "--pin");
    let rev_arg = args
        .iter()
        .position(|a| a == "--rev")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let pinned = read(&root.join(REV_FILE))?.trim().to_owned();
    let wanted = rev_arg.clone().unwrap_or_else(|| pinned.clone());

    let docs = root.join("target").join("discord-api-docs");
    let rev = fetch_docs(&docs, if wanted == "latest" { "HEAD" } else { &wanted })?;
    let areas = measure(&root, &docs.join("developers"))?;
    let report = render(&rev, &areas);
    print_summary(&rev, &areas);

    if check {
        let current = read(&root.join(REPORT)).unwrap_or_default();
        if current.replace("\r\n", "\n") != report {
            return Err(format!(
                "{REPORT} is out of date; run `cargo xtask coverage` and commit it"
            ));
        }
        return Ok(());
    }
    if rev_arg.is_some() && !pin {
        let out = root
            .join("target")
            .join(format!("coverage-{}.md", short(&rev)));
        write(&out, &report)?;
        println!("report: {}", out.display());
        if rev != pinned {
            println!("docs changed: {DOCS_REPO}/compare/{pinned}...{rev}");
        }
        return Ok(());
    }
    if pin {
        write(&root.join(REV_FILE), &format!("{rev}\n"))?;
    }
    write(&root.join(REPORT), &report)?;
    println!("wrote {REPORT}");
    Ok(())
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

fn short(rev: &str) -> &str {
    rev.get(..12).unwrap_or(rev)
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Shallow checkout of one docs commit; returns its full hash.
fn fetch_docs(dir: &Path, rev: &str) -> Result<String, String> {
    if !dir.join(".git").exists() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        git(dir, &["init", "-q"])?;
        git(dir, &["remote", "add", "origin", DOCS_REPO])?;
    }
    let have = git(dir, &["rev-parse", "HEAD"]).unwrap_or_default();
    if have != rev {
        git(dir, &["fetch", "-q", "--depth", "1", "origin", rev])?;
        git(dir, &["checkout", "-q", "--detach", "FETCH_HEAD"])?;
    }
    git(dir, &["rev-parse", "HEAD"])
}

fn measure(root: &Path, docs: &Path) -> Result<Vec<Area>, String> {
    let model = root.join("crates/model/src");
    Ok(vec![
        endpoints(root, docs)?,
        events(&model, docs)?,
        number_table(
            "Gateway opcodes",
            docs,
            "topics/opcodes-and-status-codes.mdx",
            "Gateway Opcodes",
            &enum_values(&read(&model.join("gateway/opcode.rs"))?, "OpCode"),
        )?,
        number_table(
            "Gateway close codes",
            docs,
            "topics/opcodes-and-status-codes.mdx",
            "Gateway Close Event Codes",
            &enum_values(&read(&model.join("gateway/close_code.rs"))?, "CloseCode"),
        )?,
        number_table(
            "Component types",
            docs,
            "components/reference.mdx",
            "Component Types",
            &match_arm_numbers(&read(&model.join("component.rs"))?),
        )?,
        enums(&model, docs)?,
    ])
}

// ---- docs parsing ---------------------------------------------------------

fn mdx_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            mdx_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "mdx") {
            out.push(path);
        }
    }
    Ok(())
}

/// `[\{channel.id\}](/link)` → `{}`, `[text](/link)` → `text`, no query.
fn normalize_doc_path(raw: &str) -> String {
    let mut text = String::new();
    let mut rest = raw;
    while let Some(start) = rest.find('[') {
        text.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let link = after.find("](").and_then(|mid| {
            after[mid + 2..]
                .find(')')
                .map(|end| (mid, mid + 2 + end + 1))
        });
        if let Some((mid, next)) = link {
            text.push_str(&after[..mid]);
            rest = &after[next..];
        } else {
            text.push('[');
            rest = after;
        }
    }
    text.push_str(rest);
    normalize_template(&text.replace('\\', ""))
}

/// `{anything}` → `{}`, strip a query string and trailing spaces.
fn normalize_template(path: &str) -> String {
    let path = path.split('?').next().unwrap_or(path).trim();
    let mut out = String::with_capacity(path.len());
    let mut depth = 0;
    for c in path.chars() {
        match c {
            '{' => {
                if depth == 0 {
                    out.push_str("{}");
                }
                depth += 1;
            }
            '}' => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

fn heading_text(line: &str) -> Option<&str> {
    line.starts_with('#')
        .then(|| line.trim_start_matches('#').trim())
}

/// Rows of the first table after `heading`, as trimmed cells.
fn table_after<'a>(text: &'a str, heading: &str) -> Option<Vec<Vec<&'a str>>> {
    let mut lines = text.lines();
    lines.find(|l| heading_text(l) == Some(heading))?;
    let mut rows = Vec::new();
    let mut in_table = false;
    for line in lines {
        let line = line.trim();
        if line.starts_with('|') {
            in_table = true;
            let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
            if !cells
                .iter()
                .all(|c| c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
            {
                rows.push(cells);
            }
        } else if in_table || heading_text(line).is_some() {
            break;
        }
    }
    // First row is the header.
    (!rows.is_empty()).then(|| rows.split_off(1))
}

fn strip_cell(cell: &str) -> String {
    normalize_doc_path(cell)
        .replace(['`', '*'], "")
        .trim()
        .to_owned()
}

fn read_doc(docs: &Path, page: &str) -> Result<String, String> {
    read(&docs.join(page)).map_err(|e| format!("docs page missing (docs restructured?): {e}"))
}

// ---- areas ----------------------------------------------------------------

fn endpoints(root: &Path, docs: &Path) -> Result<Area, String> {
    let ours = http_endpoints(root)?;
    let mut files = Vec::new();
    mdx_files(docs, &mut files)?;
    files.sort();
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();
    for file in files {
        // The change log quotes old and removed routes.
        if file.file_name().is_some_and(|n| n == "change-log.mdx") {
            continue;
        }
        let text = read(&file)?;
        let page = file
            .strip_prefix(docs)
            .map_or_else(|_| file.display().to_string(), |p| p.display().to_string())
            .replace('\\', "/");
        let mut heading = String::new();
        for line in text.lines() {
            if let Some(h) = heading_text(line) {
                h.clone_into(&mut heading);
            }
            let Some(start) = line.find("<Route method=\"") else {
                continue;
            };
            let rest = &line[start + 15..];
            let Some((method, rest)) = rest.split_once("\">") else {
                continue;
            };
            let Some((path, _)) = rest.split_once("</Route>") else {
                continue;
            };
            let key = format!("{method} {}", normalize_doc_path(path));
            if seen.insert(key.clone()) {
                items.push(Item {
                    covered: ours.contains(&key),
                    name: heading.clone(),
                    detail: format!("`{key}` ({page})"),
                });
            }
        }
    }
    Ok(Area {
        title: "REST endpoints",
        items,
    })
}

/// `` `METHOD /path` `` from the doc comments in `cordex-http`. Every
/// `Route::raw` template in `endpoint/` must have such a comment, so the
/// count cannot drift silently.
fn http_endpoints(root: &Path) -> Result<BTreeSet<String>, String> {
    let dir = root.join("crates/http/src/endpoint");
    let mut documented = BTreeSet::new();
    let mut templates = BTreeSet::new();
    let entries = fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let text = read(&entry.path())?;
        for line in text.lines() {
            let line = line.trim();
            if !line.starts_with("///") {
                continue;
            }
            let mut rest = line;
            while let Some(start) = rest.find('`') {
                let after = &rest[start + 1..];
                let Some(end) = after.find('`') else { break };
                let code = &after[..end];
                if let Some((method, path)) = code.split_once(' ')
                    && matches!(method, "GET" | "POST" | "PUT" | "PATCH" | "DELETE")
                    && path.starts_with('/')
                {
                    documented.insert(format!("{method} {}", normalize_template(path)));
                }
                rest = &after[end + 1..];
            }
        }
        let mut rest = text.as_str();
        while let Some(start) = rest.find("Route::raw(") {
            rest = &rest[start + 11..];
            let trimmed = rest.trim_start();
            if let Some(body) = trimmed.strip_prefix('"')
                && let Some(end) = body.find('"')
            {
                templates.insert(normalize_template(&body[..end]));
            }
        }
    }
    let paths: BTreeSet<&str> = documented
        .iter()
        .filter_map(|k| k.split_once(' ').map(|(_, p)| p))
        .collect();
    let missing: Vec<&String> = templates
        .iter()
        .filter(|t| !paths.contains(t.as_str()))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "routes without a `METHOD /path` doc comment in cordex-http: {missing:?}"
        ));
    }
    Ok(documented)
}

fn events(model: &Path, docs: &Path) -> Result<Area, String> {
    let event_rs = read(&model.join("gateway/event.rs"))?;
    let ours: BTreeSet<String> = event_rs
        .lines()
        .filter_map(|l| {
            let (_, rhs) = l.split_once("=> \"")?;
            let name = rhs.split('"').next()?;
            name.chars()
                .all(|c| c.is_ascii_uppercase() || c == '_')
                .then(|| name.to_owned())
        })
        .collect();
    let opcodes = enum_variants(&read(&model.join("gateway/opcode.rs"))?, "OpCode");
    let page = "events/gateway-events.mdx";
    let text = read_doc(docs, page)?;
    let rows = table_after(&text, "Receive Events")
        .ok_or("heading 'Receive Events' not found in gateway-events.mdx")?;
    let items = rows
        .iter()
        .filter_map(|row| row.first())
        .map(|cell| {
            let name = strip_cell(cell);
            let dispatch = name.to_uppercase().replace(' ', "_");
            let camel = name.replace(' ', "");
            let covered = ours.contains(&dispatch) || opcodes.contains(&camel);
            Item {
                covered,
                detail: format!("`{dispatch}`"),
                name,
            }
        })
        .collect();
    Ok(Area {
        title: "Gateway receive events",
        items,
    })
}

fn number_table(
    title: &'static str,
    docs: &Path,
    page: &str,
    heading: &str,
    ours: &BTreeSet<i64>,
) -> Result<Area, String> {
    Ok(Area {
        title,
        items: table_numbers(docs, page, heading)?
            .into_iter()
            .map(|(value, name)| Item {
                covered: ours.contains(&value),
                name,
                detail: format!("`{value}`"),
            })
            .collect(),
    })
}

/// (value, name) per row: the first numeric cell and the first other cell.
fn table_numbers(docs: &Path, page: &str, heading: &str) -> Result<Vec<(i64, String)>, String> {
    let text = read_doc(docs, page)?;
    let rows = table_after(&text, heading)
        .ok_or_else(|| format!("heading '{heading}' not found in {page} (docs restructured?)"))?;
    Ok(rows
        .iter()
        .filter_map(|row| {
            let cells: Vec<String> = row.iter().map(|c| strip_cell(c)).collect();
            let value = cells.iter().find_map(|c| c.parse::<i64>().ok())?;
            let name = cells
                .iter()
                .find(|c| c.parse::<i64>().is_err())
                .cloned()
                .unwrap_or_default();
            Some((value, name))
        })
        .collect())
}

fn enums(model: &Path, docs: &Path) -> Result<Area, String> {
    let mut items = Vec::new();
    for (file, name, page, heading) in ENUMS {
        let ours = enum_values(&read(&model.join(file))?, name);
        if ours.is_empty() {
            return Err(format!("enum {name} not found in {file}"));
        }
        for (value, label) in table_numbers(docs, page, heading)? {
            items.push(Item {
                covered: ours.contains(&value),
                name: format!("{name}: {label}"),
                detail: format!("`{value}`"),
            });
        }
    }
    Ok(Area {
        title: "Enum values",
        items,
    })
}

// ---- source parsing -------------------------------------------------------

/// Lines of `enum NAME … {` up to the closing `}`.
fn enum_block<'a>(text: &'a str, name: &str) -> impl Iterator<Item = &'a str> {
    let needle = format!("enum {name}");
    text.lines()
        .skip_while(move |l| {
            !l.contains(&needle)
                || l.split(&needle)
                    .nth(1)
                    .and_then(|r| r.chars().next())
                    .is_some_and(char::is_alphanumeric)
        })
        .skip(1)
        .take_while(|l| l.trim() != "}")
}

/// `Variant = 12,` values of an `int_enum!` / C-like enum.
fn enum_values(text: &str, name: &str) -> BTreeSet<i64> {
    enum_block(text, name)
        .filter_map(|l| {
            let (_, value) = l.split_once(" = ")?;
            value.trim().trim_end_matches(',').parse().ok()
        })
        .collect()
}

fn enum_variants(text: &str, name: &str) -> BTreeSet<String> {
    enum_block(text, name)
        .filter_map(|l| {
            let (variant, _) = l.trim().split_once(" = ")?;
            Some(variant.to_owned())
        })
        .collect()
}

/// `12 => …` match arms (the component type switch).
fn match_arm_numbers(text: &str) -> BTreeSet<i64> {
    text.lines()
        .filter_map(|l| l.trim().split_once(" => ")?.0.parse().ok())
        .collect()
}

// ---- output ---------------------------------------------------------------

fn counts(items: &[Item]) -> (usize, usize) {
    (items.iter().filter(|i| i.covered).count(), items.len())
}

fn percent(covered: usize, total: usize) -> String {
    if total == 0 {
        return "–".to_owned();
    }
    let tenths = covered * 1000 / total;
    format!("{}.{} %", tenths / 10, tenths % 10)
}

fn print_summary(rev: &str, areas: &[Area]) {
    println!("discord-api-docs {}", short(rev));
    let (mut c, mut t) = (0, 0);
    for area in areas {
        let (covered, total) = counts(&area.items);
        c += covered;
        t += total;
        println!(
            "  {:<24} {covered:>4}/{total:<4} {}",
            area.title,
            percent(covered, total)
        );
    }
    println!(
        "  {:<24} {c:>4}/{t:<4} {}",
        "total (measured)",
        percent(c, t)
    );
}

fn render(rev: &str, areas: &[Area]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# API coverage\n\n\
         Generated by `cargo xtask coverage` against \
         [`discord-api-docs@{short}`]({DOCS_REPO}/tree/{rev}). Do not edit by hand.\n\n\
         Measured: REST endpoints, gateway receive events, opcodes and close codes, \
         component types and the value tables of the model enums. **Not measured yet:** \
         object fields and bitflags, so the total below is not the full API.\n\n\
         | Area | Covered | Total | % |\n|---|---:|---:|---:|",
        short = short(rev)
    );
    let (mut c, mut t) = (0, 0);
    for area in areas {
        let (covered, total) = counts(&area.items);
        c += covered;
        t += total;
        let _ = writeln!(
            out,
            "| {} | {covered} | {total} | {} |",
            area.title,
            percent(covered, total)
        );
    }
    let _ = writeln!(
        out,
        "| **Total (measured)** | **{c}** | **{t}** | **{}** |",
        percent(c, t)
    );
    for area in areas {
        let (covered, total) = counts(&area.items);
        let _ = writeln!(out, "\n## {} ({covered}/{total})\n", area.title);
        let missing: Vec<&Item> = area.items.iter().filter(|i| !i.covered).collect();
        if missing.is_empty() {
            let _ = writeln!(out, "Everything covered.");
            continue;
        }
        let _ = writeln!(out, "Missing:\n");
        for item in missing {
            let _ = writeln!(out, "- {} {}", item.name, item.detail);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_paths_normalize_like_templates() {
        let doc = r"/channels/[\{channel.id\}](/developers/resources/channel#channel-object)/messages/[\{message.id\}](/x)";
        assert_eq!(normalize_doc_path(doc), "/channels/{}/messages/{}");
        assert_eq!(
            normalize_template("/channels/{channel_id}/messages/{message_id}"),
            "/channels/{}/messages/{}"
        );
        assert_eq!(
            normalize_template("/guilds/{guild_id}/members/search?query"),
            "/guilds/{}/members/search"
        );
    }

    #[test]
    fn tables_and_enums_parse() {
        let doc = "###### Button Styles\n\n| Name | Value | Note |\n|---|---|---|\n| Primary | `1` | x |\n| [Link](/a) | 5 | y |\n\n## Next\n";
        let rows = table_after(doc, "Button Styles").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(strip_cell(rows[1][0]), "Link");

        let src = "int_enum! {\n    pub enum ButtonStyle: u8 {\n        /// x\n        Primary = 1,\n        Link = 5,\n    }\n}\npub enum ButtonStyleX: u8 {\n    Other = 9,\n}\n";
        assert_eq!(enum_values(src, "ButtonStyle"), BTreeSet::from([1, 5]));
        assert_eq!(
            match_arm_numbers("  1 => a,\n  17 => b,\n x => c"),
            BTreeSet::from([1, 17])
        );
    }
}
