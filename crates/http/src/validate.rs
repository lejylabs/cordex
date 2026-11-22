//! Local builder limits (PLAN.md §6.2), checked before the request is sent.

use drs_model::message::Embed;

use crate::error::HttpError;

/// Discord message content limit.
pub const CONTENT_LIMIT: usize = 2000;
/// Maximum embeds on one message.
pub const EMBED_COUNT_LIMIT: usize = 10;
/// Combined text across all embeds of a message.
pub const EMBED_TOTAL_LIMIT: usize = 6000;
/// Maximum files on one message.
pub const FILE_COUNT_LIMIT: usize = 10;
/// Conservative per-file cap (non-Nitro).
pub const FILE_SIZE_LIMIT: usize = 25 * 1024 * 1024;
/// Stickers on a message.
pub const STICKER_COUNT_LIMIT: usize = 3;

pub(crate) fn content_len(content: &str) -> Result<(), HttpError> {
    let n = content.chars().count();
    if n > CONTENT_LIMIT {
        Err(HttpError::validation(
            format!("message content is {n} characters; max {CONTENT_LIMIT}").into_boxed_str(),
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn embed_limits(embeds: &[Embed]) -> Result<(), HttpError> {
    if embeds.len() > EMBED_COUNT_LIMIT {
        return Err(HttpError::validation(
            format!(
                "a message can have at most {EMBED_COUNT_LIMIT} embeds (got {})",
                embeds.len()
            )
            .into_boxed_str(),
        ));
    }
    let total: usize = embeds.iter().map(embed_text_len).sum();
    if total > EMBED_TOTAL_LIMIT {
        return Err(HttpError::validation(
            format!("embed text is {total} characters; max {EMBED_TOTAL_LIMIT}").into_boxed_str(),
        ));
    }
    Ok(())
}

fn embed_text_len(embed: &Embed) -> usize {
    let mut n = 0;
    n += chars(embed.title.as_deref());
    n += chars(embed.description.as_deref());
    if let Some(footer) = &embed.footer {
        n += footer.text.chars().count();
    }
    if let Some(author) = &embed.author {
        n += author.name.chars().count();
    }
    for field in &embed.fields {
        n += field.name.chars().count();
        n += field.value.chars().count();
    }
    n
}

fn chars(s: Option<&str>) -> usize {
    s.map_or(0, |s| s.chars().count())
}
