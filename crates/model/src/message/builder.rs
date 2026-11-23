//! Safe Discord Markdown message builder (inspired by Serenity's `MessageBuilder`).

use std::fmt::{self, Write};

use crate::mention::Mentionable;
use crate::timestamp::{Timestamp, TimestampStyle};

/// Builds a Discord message string with Markdown helpers and safe escaping
/// for user-supplied text.
///
/// ```
/// use cordex_model::message::MessageBuilder;
///
/// let text = MessageBuilder::new()
///     .push("Hello ")
///     .push_bold_safe("*world*")
///     .push_line("!")
///     .push_codeblock("let x = 1;", Some("rust"))
///     .build();
/// assert!(text.contains(r"**\*world\***"));
/// assert!(text.contains("```rust\nlet x = 1;\n```"));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MessageBuilder(String);

impl MessageBuilder {
    /// Empty message builder.
    pub fn new() -> Self {
        Self(String::new())
    }

    /// Empty message builder with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self(String::with_capacity(capacity))
    }

    /// Append raw text as-is.
    pub fn push(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.0.push_str(text.as_ref());
        self
    }

    /// Append raw text followed by a newline.
    pub fn push_line(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.0.push_str(text.as_ref());
        self.0.push('\n');
        self
    }

    /// Append text with Discord Markdown characters escaped (`\`, `*`, `_`,
    /// `~`, `` ` ``, `|`, `>`, `[`, `]`, `(`, `)`).
    pub fn push_safe(&mut self, text: impl AsRef<str>) -> &mut Self {
        push_escaped(&mut self.0, text.as_ref());
        self
    }

    /// Append Markdown-escaped text followed by a newline.
    pub fn push_line_safe(&mut self, text: impl AsRef<str>) -> &mut Self {
        push_escaped(&mut self.0, text.as_ref());
        self.0.push('\n');
        self
    }

    /// Append `**bold**` text.
    pub fn push_bold(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.0.push_str("**");
        self.0.push_str(text.as_ref());
        self.0.push_str("**");
        self
    }

    /// Append `**bold**` text with Markdown characters inside escaped.
    pub fn push_bold_safe(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.0.push_str("**");
        push_escaped(&mut self.0, text.as_ref());
        self.0.push_str("**");
        self
    }

    /// Append `*italic*` text.
    pub fn push_italic(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.0.push('*');
        self.0.push_str(text.as_ref());
        self.0.push('*');
        self
    }

    /// Append `*italic*` text with Markdown characters inside escaped.
    pub fn push_italic_safe(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.0.push('*');
        push_escaped(&mut self.0, text.as_ref());
        self.0.push('*');
        self
    }

    /// Append `__underlined__` text.
    pub fn push_underline(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.0.push_str("__");
        self.0.push_str(text.as_ref());
        self.0.push_str("__");
        self
    }

    /// Append `__underlined__` text with Markdown characters inside escaped.
    pub fn push_underline_safe(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.0.push_str("__");
        push_escaped(&mut self.0, text.as_ref());
        self.0.push_str("__");
        self
    }

    /// Append `~~strikethrough~~` text.
    pub fn push_strike(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.0.push_str("~~");
        self.0.push_str(text.as_ref());
        self.0.push_str("~~");
        self
    }

    /// Append `~~strikethrough~~` text with Markdown characters inside escaped.
    pub fn push_strike_safe(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.0.push_str("~~");
        push_escaped(&mut self.0, text.as_ref());
        self.0.push_str("~~");
        self
    }

    /// Append `||spoiler||` text.
    pub fn push_spoiler(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.0.push_str("||");
        self.0.push_str(text.as_ref());
        self.0.push_str("||");
        self
    }

    /// Append `||spoiler||` text with Markdown characters inside escaped.
    pub fn push_spoiler_safe(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.0.push_str("||");
        push_escaped(&mut self.0, text.as_ref());
        self.0.push_str("||");
        self
    }

    /// Append `` `inline code` ``.
    pub fn push_mono(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.0.push('`');
        self.0.push_str(text.as_ref());
        self.0.push('`');
        self
    }

    /// Append `` `inline code` `` with backticks inside replaced safely.
    pub fn push_mono_safe(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.0.push('`');
        for ch in text.as_ref().chars() {
            if ch == '`' {
                self.0.push('\'');
            } else {
                self.0.push(ch);
            }
        }
        self.0.push('`');
        self
    }

    /// Append a quoted line (`> text\n`).
    pub fn push_quote_line(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.0.push_str("> ");
        self.0.push_str(text.as_ref());
        self.0.push('\n');
        self
    }

    /// Append a fenced code block (```` ```lang\ncode\n``` ````).
    pub fn push_codeblock(&mut self, code: impl AsRef<str>, language: Option<&str>) -> &mut Self {
        self.0.push_str("```");
        if let Some(lang) = language {
            self.0.push_str(lang);
        }
        self.0.push('\n');
        self.0.push_str(code.as_ref());
        if !code.as_ref().ends_with('\n') {
            self.0.push('\n');
        }
        self.0.push_str("```");
        self
    }

    /// Append a Markdown link `[label](url)`.
    pub fn push_named_link(&mut self, label: impl AsRef<str>, url: impl AsRef<str>) -> &mut Self {
        self.0.push('[');
        self.0.push_str(label.as_ref());
        self.0.push_str("](");
        self.0.push_str(url.as_ref());
        self.0.push(')');
        self
    }

    /// Append a Markdown link `[label](url)` with `label` escaped.
    pub fn push_named_link_safe(
        &mut self,
        label: impl AsRef<str>,
        url: impl AsRef<str>,
    ) -> &mut Self {
        self.0.push('[');
        push_escaped(&mut self.0, label.as_ref());
        self.0.push_str("](");
        self.0.push_str(url.as_ref());
        self.0.push(')');
        self
    }

    /// Append a mention (`<@id>`, `<#id>`, `<@&id>`, custom emoji, …).
    pub fn mention(&mut self, target: &impl Mentionable) -> &mut Self {
        let _ = write!(self.0, "{}", target.mention());
        self
    }

    /// Append a Discord timestamp (`<t:unix:style>`).
    pub fn push_timestamp(&mut self, timestamp: Timestamp, style: TimestampStyle) -> &mut Self {
        self.0.push_str(&timestamp.markdown(style));
        self
    }

    /// Current contents as `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Finish and return the built `String`.
    pub fn build(&mut self) -> String {
        std::mem::take(&mut self.0)
    }

    /// Consume the builder and return the built `String`.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for MessageBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<MessageBuilder> for String {
    fn from(builder: MessageBuilder) -> Self {
        builder.0
    }
}

fn push_escaped(out: &mut String, text: &str) {
    out.reserve(text.len());
    for ch in text.chars() {
        if matches!(
            ch,
            '\\' | '*' | '_' | '~' | '`' | '|' | '>' | '[' | ']' | '(' | ')'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Id;
    use crate::id::marker::UserMarker;

    #[test]
    fn builds_and_escapes_markdown() {
        let uid = Id::<UserMarker>::new(42).unwrap();
        let out = MessageBuilder::new()
            .push("Hi ")
            .mention(&uid)
            .push(" — ")
            .push_bold_safe("*admin* [test]")
            .push_line("")
            .push_spoiler_safe("a||b")
            .push_line("")
            .push_codeblock("fn main() {}", Some("rs"))
            .build();
        assert!(out.starts_with("Hi <@42> — **\\*admin\\* \\[test\\]**\n"));
        assert!(out.contains("||a\\|\\|b||\n"));
        assert!(out.ends_with("```rs\nfn main() {}\n```"));
    }
}
