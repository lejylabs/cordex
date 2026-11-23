//! Short constructors for the common components.
//!
//! ```
//! use cordex_model::component::{ActionRow, Button, Label, TextInput};
//!
//! let row = ActionRow::new([
//!     Button::primary("vote:yes", "Yes"),
//!     Button::danger("vote:no", "No"),
//!     Button::link("https://discord.com", "Docs"),
//! ]);
//! let field = Label::new("Your name", TextInput::short("name").max_length(32));
//! # let _ = (row, field);
//! ```

use super::{
    ActionRow, Button, ButtonStyle, Component, Label, SelectOption, StringSelect, TextInput,
    TextInputStyle,
};

macro_rules! into_component {
    ($($ty:ident),+) => {$(
        impl From<$ty> for Component {
            fn from(value: $ty) -> Self {
                Self::$ty(value)
            }
        }
    )+};
}

into_component!(ActionRow, Button, StringSelect, TextInput, Label);

impl ActionRow {
    /// Row with up to 5 buttons or one select.
    pub fn new<C: Into<Component>>(components: impl IntoIterator<Item = C>) -> Self {
        Self {
            id: None,
            components: components.into_iter().map(Into::into).collect(),
        }
    }
}

impl Button {
    fn with_id(style: ButtonStyle, custom_id: &str, label: &str) -> Self {
        Self {
            id: None,
            style,
            label: Some(label.into()),
            emoji: None,
            custom_id: Some(custom_id.into()),
            sku_id: None,
            url: None,
            disabled: false,
        }
    }

    /// Blurple button; presses go to `Bot::on_button(custom_id, …)`.
    pub fn primary(custom_id: impl AsRef<str>, label: impl AsRef<str>) -> Self {
        Self::with_id(ButtonStyle::Primary, custom_id.as_ref(), label.as_ref())
    }

    /// Grey button.
    pub fn secondary(custom_id: impl AsRef<str>, label: impl AsRef<str>) -> Self {
        Self::with_id(ButtonStyle::Secondary, custom_id.as_ref(), label.as_ref())
    }

    /// Green button.
    pub fn success(custom_id: impl AsRef<str>, label: impl AsRef<str>) -> Self {
        Self::with_id(ButtonStyle::Success, custom_id.as_ref(), label.as_ref())
    }

    /// Red button.
    pub fn danger(custom_id: impl AsRef<str>, label: impl AsRef<str>) -> Self {
        Self::with_id(ButtonStyle::Danger, custom_id.as_ref(), label.as_ref())
    }

    /// Button that opens a URL (no interaction is sent).
    pub fn link(url: impl AsRef<str>, label: impl AsRef<str>) -> Self {
        Self {
            custom_id: None,
            url: Some(url.as_ref().into()),
            ..Self::with_id(ButtonStyle::Link, "", label.as_ref())
        }
    }

    /// Show the button greyed out.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

impl StringSelect {
    /// Select menu with predefined options.
    pub fn new(
        custom_id: impl AsRef<str>,
        options: impl IntoIterator<Item = SelectOption>,
    ) -> Self {
        Self {
            id: None,
            custom_id: custom_id.as_ref().into(),
            options: options.into_iter().collect(),
            placeholder: None,
            min_values: None,
            max_values: None,
            required: None,
            disabled: false,
        }
    }

    /// Text shown while nothing is selected.
    #[must_use]
    pub fn placeholder(mut self, text: impl AsRef<str>) -> Self {
        self.placeholder = Some(text.as_ref().into());
        self
    }

    /// Allowed number of selections (default 1..=1).
    #[must_use]
    pub fn values(mut self, min: u8, max: u8) -> Self {
        self.min_values = Some(min);
        self.max_values = Some(max);
        self
    }
}

impl SelectOption {
    /// Option with a shown label and the value the bot receives.
    pub fn new(label: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        Self {
            label: label.as_ref().into(),
            value: value.as_ref().into(),
            description: None,
            emoji: None,
            default: false,
        }
    }

    /// Smaller text below the label.
    #[must_use]
    pub fn description(mut self, text: impl AsRef<str>) -> Self {
        self.description = Some(text.as_ref().into());
        self
    }
}

impl TextInput {
    fn with_style(style: TextInputStyle, custom_id: &str) -> Self {
        Self {
            id: None,
            custom_id: custom_id.into(),
            style,
            min_length: None,
            max_length: None,
            required: None,
            value: None,
            placeholder: None,
        }
    }

    /// Single-line text input; read it with `ModalContext::value(custom_id)`.
    /// Wrap it in a [`Label`] for the modal.
    pub fn short(custom_id: impl AsRef<str>) -> Self {
        Self::with_style(TextInputStyle::Short, custom_id.as_ref())
    }

    /// Multi-line text input.
    pub fn paragraph(custom_id: impl AsRef<str>) -> Self {
        Self::with_style(TextInputStyle::Paragraph, custom_id.as_ref())
    }

    /// Greyed-out hint text.
    #[must_use]
    pub fn placeholder(mut self, text: impl AsRef<str>) -> Self {
        self.placeholder = Some(text.as_ref().into());
        self
    }

    /// Prefilled text.
    #[must_use]
    pub fn value(mut self, text: impl AsRef<str>) -> Self {
        self.value = Some(text.as_ref().into());
        self
    }

    /// Allow an empty input (required by default).
    #[must_use]
    pub fn optional(mut self) -> Self {
        self.required = Some(false);
        self
    }

    /// Minimum length (0–4000).
    #[must_use]
    pub fn min_length(mut self, len: u16) -> Self {
        self.min_length = Some(len);
        self
    }

    /// Maximum length (1–4000).
    #[must_use]
    pub fn max_length(mut self, len: u16) -> Self {
        self.max_length = Some(len);
        self
    }
}

impl Label {
    /// Label (max. 45 characters) around one modal component.
    pub fn new(label: impl AsRef<str>, component: impl Into<Component>) -> Self {
        Self {
            id: None,
            label: label.as_ref().into(),
            description: None,
            component: Box::new(component.into()),
        }
    }

    /// Smaller text below the label (max. 100 characters).
    #[must_use]
    pub fn description(mut self, text: impl AsRef<str>) -> Self {
        self.description = Some(text.as_ref().into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn row_of_buttons_serializes_like_discord_expects() {
        let row: Component = ActionRow::new([
            Button::primary("vote:yes", "Yes"),
            Button::link("https://example.com", "Site").disabled(),
        ])
        .into();
        assert_eq!(
            serde_json::to_value(&row).unwrap(),
            json!({
                "type": 1,
                "components": [
                    {"type": 2, "style": 1, "label": "Yes", "custom_id": "vote:yes"},
                    {"type": 2, "style": 5, "label": "Site", "url": "https://example.com", "disabled": true}
                ]
            })
        );
    }

    #[test]
    fn label_wraps_text_input() {
        let label: Component =
            Label::new("Name", TextInput::short("name").max_length(32).optional())
                .description("How we call you")
                .into();
        assert_eq!(
            serde_json::to_value(&label).unwrap(),
            json!({
                "type": 18,
                "label": "Name",
                "description": "How we call you",
                "component": {"type": 4, "custom_id": "name", "style": 1, "max_length": 32, "required": false}
            })
        );
    }

    #[test]
    fn string_select() {
        let select: Component = StringSelect::new(
            "color",
            [SelectOption::new("Red", "red").description("warm")],
        )
        .placeholder("Pick")
        .values(1, 1)
        .into();
        let v = serde_json::to_value(&select).unwrap();
        assert_eq!(v["type"], 3);
        assert_eq!(v["options"][0]["value"], "red");
        assert_eq!(v["min_values"], 1);
    }
}
