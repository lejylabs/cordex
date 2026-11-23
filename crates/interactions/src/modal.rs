//! Modal builder.
//!
//! ```
//! use cordex_interactions::Modal;
//!
//! let modal = Modal::new("feedback", "Feedback")
//!     .text_input("subject", "Subject")
//!     .paragraph("text", "What do you think?");
//! assert!(modal.validate().is_ok());
//! ```

use cordex_model::component::{Component, Label, TextInput};

use crate::error::InteractionError;

/// Max. top-level components in a modal.
pub const MAX_MODAL_COMPONENTS: usize = 5;

/// A modal (popup form). Open it with
/// [`InteractionHandle::show_modal`](crate::InteractionHandle::show_modal);
/// the submit arrives as a `MODAL_SUBMIT` interaction with the same
/// `custom_id`.
#[derive(Debug, Clone, PartialEq)]
pub struct Modal {
    pub(crate) custom_id: Box<str>,
    pub(crate) title: Box<str>,
    pub(crate) components: Vec<Component>,
}

impl Modal {
    /// Modal with a `custom_id` (max. 100 characters) and a title (max. 45).
    pub fn new(custom_id: impl AsRef<str>, title: impl AsRef<str>) -> Self {
        Self {
            custom_id: custom_id.as_ref().into(),
            title: title.as_ref().into(),
            components: Vec::new(),
        }
    }

    /// Required single-line text field with a label.
    #[must_use]
    pub fn text_input(self, custom_id: impl AsRef<str>, label: impl AsRef<str>) -> Self {
        self.component(Label::new(label, TextInput::short(custom_id)))
    }

    /// Required multi-line text field with a label.
    #[must_use]
    pub fn paragraph(self, custom_id: impl AsRef<str>, label: impl AsRef<str>) -> Self {
        self.component(Label::new(label, TextInput::paragraph(custom_id)))
    }

    /// Any component, e.g. a [`Label`] around a configured [`TextInput`]
    /// (`TextInput::short("age").optional().max_length(3)`).
    #[must_use]
    pub fn component(mut self, component: impl Into<Component>) -> Self {
        self.components.push(component.into());
        self
    }

    /// `custom_id` of the modal.
    pub fn custom_id(&self) -> &str {
        &self.custom_id
    }

    /// Check Discord's limits locally.
    pub fn validate(&self) -> Result<(), InteractionError> {
        let invalid = |msg: String| Err(InteractionError::InvalidResponse(msg.into()));
        if !(1..=100).contains(&self.custom_id.chars().count()) {
            return invalid(format!(
                "modal custom_id '{}' must be 1–100 characters",
                self.custom_id
            ));
        }
        if !(1..=45).contains(&self.title.chars().count()) {
            return invalid(format!(
                "modal title '{}' must be 1–45 characters",
                self.title
            ));
        }
        if !(1..=MAX_MODAL_COMPONENTS).contains(&self.components.len()) {
            return invalid(format!(
                "modal '{}' needs 1–{MAX_MODAL_COMPONENTS} components, has {}",
                self.custom_id,
                self.components.len()
            ));
        }
        for component in &self.components {
            if let Component::Label(label) = component
                && !(1..=45).contains(&label.label.chars().count())
            {
                return invalid(format!(
                    "modal label '{}' must be 1–45 characters",
                    label.label
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_are_checked() {
        assert!(Modal::new("m", "T").text_input("a", "A").validate().is_ok());

        let err = Modal::new("m", "T").validate().unwrap_err();
        assert!(err.to_string().contains("needs 1–5 components"), "{err}");

        let err = Modal::new("m", "x".repeat(46))
            .text_input("a", "A")
            .validate()
            .unwrap_err();
        assert!(err.to_string().contains("title"), "{err}");

        let mut six = Modal::new("m", "T");
        for i in 0..6 {
            six = six.text_input(format!("f{i}"), "F");
        }
        assert!(six.validate().is_err());

        let err = Modal::new("m", "T")
            .text_input("a", "y".repeat(46))
            .validate()
            .unwrap_err();
        assert!(err.to_string().contains("label"), "{err}");
    }
}
