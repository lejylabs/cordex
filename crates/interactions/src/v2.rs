//! Fluent Components V2 layout builder with local Discord limit validation.
//!
//! Produces `Vec<Component>` or a [`Reply`] (with `IS_COMPONENTS_V2` set
//! automatically via [`Reply::layout`]).

use cordex_model::component::{
    Component, Container, FileComponent, MediaGallery, Section, Separator, SeparatorSpacing,
    TextDisplay, Thumbnail,
};
use thiserror::Error;

use crate::reply::Reply;

/// Maximum number of components Discord allows in a single Components V2 message.
pub const MAX_V2_COMPONENTS: usize = 40;

/// Maximum combined character count across all [`TextDisplay`] components.
pub const MAX_V2_TEXT_CHARS: usize = 4000;

/// Validation errors for [`LayoutBuilder`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LayoutError {
    /// More than [`MAX_V2_COMPONENTS`] components in a single message.
    #[error("layout has {count} components (max {MAX_V2_COMPONENTS})")]
    TooManyComponents {
        /// Actual total component count.
        count: usize,
    },
    /// Combined [`TextDisplay`] content exceeds [`MAX_V2_TEXT_CHARS`].
    #[error("layout text is {chars} chars (max {MAX_V2_TEXT_CHARS})")]
    TextTooLong {
        /// Actual combined Unicode char count.
        chars: usize,
    },
    /// A layout must contain at least one component.
    #[error("layout must contain at least one component")]
    Empty,
}

/// Fluent builder for a Components V2 message layout.
#[derive(Debug, Clone, Default)]
pub struct LayoutBuilder {
    components: Vec<Component>,
}

impl LayoutBuilder {
    /// Start an empty Components V2 layout.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a [`TextDisplay`] markdown block.
    #[must_use]
    pub fn text(mut self, content: impl AsRef<str>) -> Self {
        self.components
            .push(Component::TextDisplay(TextDisplay::new(content)));
        self
    }

    /// Append a [`Separator`] divider.
    #[must_use]
    pub fn separator(mut self, divider: bool, spacing: SeparatorSpacing) -> Self {
        let mut sep = Separator::new();
        sep.divider = Some(divider);
        sep.spacing = Some(spacing);
        self.components.push(Component::Separator(sep));
        self
    }

    /// Append a [`Section`] with text blocks and an accessory (e.g. [`Thumbnail`] or button).
    #[must_use]
    pub fn section<S: AsRef<str>>(
        mut self,
        texts: impl IntoIterator<Item = S>,
        accessory: impl Into<Component>,
    ) -> Self {
        let items = texts
            .into_iter()
            .map(|t| Component::TextDisplay(TextDisplay::new(t)));
        self.components
            .push(Component::Section(Section::new(items, accessory)));
        self
    }

    /// Append a [`MediaGallery`] from image/video URLs.
    #[must_use]
    pub fn media_gallery<S: AsRef<str>>(mut self, urls: impl IntoIterator<Item = S>) -> Self {
        self.components
            .push(Component::MediaGallery(MediaGallery::new(urls)));
        self
    }

    /// Append a [`FileComponent`] referencing an attachment URL (e.g. `attachment://report.txt`).
    #[must_use]
    pub fn file(mut self, url: impl AsRef<str>, spoiler: bool) -> Self {
        let mut f = FileComponent::new(url);
        if spoiler {
            f = f.spoiler();
        }
        self.components.push(Component::File(f));
        self
    }

    /// Append a [`Container`] built via a nested [`LayoutBuilder`].
    #[must_use]
    pub fn container(
        mut self,
        accent_color: Option<u32>,
        spoiler: bool,
        build: impl FnOnce(Self) -> Self,
    ) -> Self {
        let inner = build(Self::new());
        let mut ct = Container::new(inner.components);
        if let Some(color) = accent_color {
            ct = ct.accent_color(color);
        }
        if spoiler {
            ct = ct.spoiler();
        }
        self.components.push(Component::Container(ct));
        self
    }

    /// Append any raw [`Component`] (e.g. an [`ActionRow`](cordex_model::component::ActionRow)).
    #[must_use]
    pub fn push(mut self, component: impl Into<Component>) -> Self {
        self.components.push(component.into());
        self
    }

    /// Helper to build a [`Thumbnail`] accessory component for [`Self::section`].
    pub fn thumbnail(url: impl AsRef<str>) -> Component {
        Component::Thumbnail(Thumbnail::new(url))
    }

    /// Validate local Discord limits and return the built components.
    pub fn build(self) -> Result<Vec<Component>, LayoutError> {
        if self.components.is_empty() {
            return Err(LayoutError::Empty);
        }
        let mut count = 0usize;
        let mut chars = 0usize;
        count_components(&self.components, &mut count, &mut chars);
        if count > MAX_V2_COMPONENTS {
            return Err(LayoutError::TooManyComponents { count });
        }
        if chars > MAX_V2_TEXT_CHARS {
            return Err(LayoutError::TextTooLong { chars });
        }
        Ok(self.components)
    }

    /// Validate and convert directly into a [`Reply`] with `IS_COMPONENTS_V2` enabled.
    pub fn into_reply(self) -> Result<Reply, LayoutError> {
        Ok(Reply::layout(self.build()?))
    }
}

fn count_components(items: &[Component], count: &mut usize, chars: &mut usize) {
    for c in items {
        *count += 1;
        match c {
            Component::TextDisplay(t) => *chars += t.content.chars().count(),
            Component::Container(ct) => count_components(&ct.components, count, chars),
            Component::Section(s) => {
                count_components(&s.components, count, chars);
                *count += 1;
            }
            Component::ActionRow(row) => *count += row.components.len(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_builder_validates_and_builds_reply() {
        let reply = LayoutBuilder::new()
            .container(Some(0x0058_65F2), false, |c| {
                c.text("## Status")
                    .separator(true, SeparatorSpacing::Small)
                    .text("All systems operational.")
            })
            .into_reply();
        assert!(reply.is_ok());

        let too_long = LayoutBuilder::new()
            .text("a".repeat(MAX_V2_TEXT_CHARS + 1))
            .build();
        assert_eq!(
            too_long,
            Err(LayoutError::TextTooLong {
                chars: MAX_V2_TEXT_CHARS + 1
            })
        );
    }
}
