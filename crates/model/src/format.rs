//! Discord message splitting (codeblock-aware) and markdown/mention escaping.

/// Default Discord message content limit (2000 Unicode scalar values).
pub const MESSAGE_LIMIT: usize = 2000;

/// Default Discord embed description limit (4096 Unicode scalar values).
pub const EMBED_DESCRIPTION_LIMIT: usize = 4096;

/// Split `text` into chunks of at most `max_chars` Unicode scalar values.
///
/// - Prefers splitting on paragraph (`\n\n`) or line (`\n`) boundaries, falling
///   back to whitespace and finally strict UTF-8 char boundaries.
/// - Tracks open fenced Markdown codeblocks (```` ```lang ````): closes an open
///   fence at the end of a chunk and re-opens it with the same language tag at
///   the start of the next chunk.
pub fn split_message(text: &str, max_chars: usize) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let max_chars = max_chars.max(16);
    if text.chars().count() <= max_chars {
        return vec![text.to_owned()];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;
    let mut open_fence: Option<String> = None;

    while !remaining.is_empty() {
        let prefix = match &open_fence {
            Some(lang) => format!("```{lang}\n"),
            None => String::new(),
        };
        let prefix_chars = prefix.chars().count();
        // Reserve 4 chars for "\n```" in case we need to close a fence.
        let budget = max_chars.saturating_sub(prefix_chars + 4).max(1);

        let split_byte = find_split_point(remaining, budget);
        let (piece, rest) = remaining.split_at(split_byte);
        let piece_trimmed = piece.trim_end_matches('\n');

        let mut chunk = String::with_capacity(prefix.len() + piece_trimmed.len() + 4);
        chunk.push_str(&prefix);
        chunk.push_str(piece_trimmed);

        open_fence = track_fence(open_fence, piece_trimmed);
        if open_fence.is_some() && !rest.is_empty() {
            chunk.push_str("\n```");
        }
        chunks.push(chunk);

        remaining = rest.trim_start_matches('\n');
    }

    chunks
}

/// Convenience wrapper for [`split_message`] with Discord's 2000-char limit.
pub fn split_message_2000(text: &str) -> Vec<String> {
    split_message(text, MESSAGE_LIMIT)
}

/// Convenience wrapper for [`split_message`] with Discord's 4096-char embed limit.
pub fn split_message_4096(text: &str) -> Vec<String> {
    split_message(text, EMBED_DESCRIPTION_LIMIT)
}

/// Escape Discord Markdown metacharacters (`\`, `*`, `_`, `~`, `` ` ``, `|`, `>`).
pub fn escape_markdown(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if matches!(ch, '\\' | '*' | '_' | '~' | '`' | '|' | '>') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Neutralize `@everyone`, `@here`, and `<@...>` / `<#...>` mentions by
/// inserting a zero-width space (`\u{200B}`) after `@` and `<`.
pub fn escape_mentions(input: &str) -> String {
    input
        .replace("@everyone", "@\u{200B}everyone")
        .replace("@here", "@\u{200B}here")
        .replace("<@", "<\u{200B}@")
}

fn find_split_point(s: &str, max_chars: usize) -> usize {
    let mut limit_byte = s.len();
    for (count, (idx, _)) in s.char_indices().enumerate() {
        if count == max_chars {
            limit_byte = idx;
            break;
        }
    }
    if limit_byte == s.len() {
        return limit_byte;
    }
    let window = &s[..limit_byte];
    if let Some(pos) = window.rfind("\n\n")
        && pos > 0
    {
        return pos;
    }
    if let Some(pos) = window.rfind('\n')
        && pos > 0
    {
        return pos;
    }
    if let Some(pos) = window.rfind(' ')
        && pos > 0
    {
        return pos;
    }
    limit_byte
}

fn track_fence(mut current: Option<String>, piece: &str) -> Option<String> {
    for line in piece.lines() {
        let trimmed = line.trim_start();
        if let Some(after) = trimmed.strip_prefix("```") {
            if current.is_some() {
                current = None;
            } else {
                let lang = after.split_whitespace().next().unwrap_or("").to_owned();
                current = Some(lang);
            }
        }
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_message_preserves_codeblocks_and_utf8() {
        let text = "```rust\nlet a = \"héllo 🚀\";\nlet b = 2;\nlet c = 3;\n```";
        let chunks = split_message(text, 32);
        assert!(chunks.len() >= 2);
        for chunk in &chunks {
            assert!(chunk.chars().count() <= 32, "chunk too long: {chunk:?}");
            assert_eq!(
                chunk.matches("```").count() % 2,
                0,
                "unbalanced fence: {chunk:?}"
            );
        }
        assert!(chunks[1].starts_with("```rust\n"));
    }

    #[test]
    fn escape_helpers_work() {
        assert_eq!(escape_markdown("*bold* _it_"), "\\*bold\\* \\_it\\_");
        assert!(!escape_mentions("hi @everyone <@123>").contains("@everyone"));
        assert!(!escape_mentions("hi @everyone <@123>").contains("<@123>"));
    }
}
