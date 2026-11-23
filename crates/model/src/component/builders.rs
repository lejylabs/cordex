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
    ActionRow, Button, ButtonStyle, Checkbox, CheckboxGroup, ChoiceOption, Component, Container,
    FileComponent, FileUpload, Label, MediaGallery, MediaGalleryItem, RadioGroup, Section,
    SelectDefaultValue, SelectDefaultValueType, SelectMenu, SelectOption, Separator,
    SeparatorSpacing, StringSelect, TextDisplay, TextInput, TextInputStyle, Thumbnail,
    UnfurledMediaItem,
};
use crate::channel::ChannelType;
use crate::id::Id;
use crate::id::marker::GenericMarker;

macro_rules! into_component {
    ($($ty:ident),+) => {$(
        impl From<$ty> for Component {
            fn from(value: $ty) -> Self {
                Self::$ty(value)
            }
        }
    )+};
}

into_component!(
    ActionRow,
    Button,
    StringSelect,
    TextInput,
    Label,
    Section,
    TextDisplay,
    Thumbnail,
    MediaGallery,
    Separator,
    Container,
    FileUpload,
    RadioGroup,
    CheckboxGroup,
    Checkbox
);

impl From<FileComponent> for Component {
    fn from(value: FileComponent) -> Self {
        Self::File(value)
    }
}

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

/// Which entity a [`SelectMenu`] picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectKind {
    /// Users.
    User,
    /// Roles.
    Role,
    /// Users and roles.
    Mentionable,
    /// Channels.
    Channel,
}

/// A user, role, mentionable or channel select; converts into a
/// [`Component`] of the right type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitySelect {
    kind: SelectKind,
    menu: SelectMenu,
}

impl SelectMenu {
    fn entity(kind: SelectKind, custom_id: &str) -> EntitySelect {
        EntitySelect {
            kind,
            menu: Self {
                id: None,
                custom_id: custom_id.into(),
                channel_types: Vec::new(),
                placeholder: None,
                default_values: Vec::new(),
                min_values: None,
                max_values: None,
                required: None,
                disabled: false,
            },
        }
    }

    /// User select.
    pub fn users(custom_id: impl AsRef<str>) -> EntitySelect {
        Self::entity(SelectKind::User, custom_id.as_ref())
    }

    /// Role select.
    pub fn roles(custom_id: impl AsRef<str>) -> EntitySelect {
        Self::entity(SelectKind::Role, custom_id.as_ref())
    }

    /// User and role select.
    pub fn mentionables(custom_id: impl AsRef<str>) -> EntitySelect {
        Self::entity(SelectKind::Mentionable, custom_id.as_ref())
    }

    /// Channel select (limit the types with [`EntitySelect::channel_types`]).
    pub fn channels(custom_id: impl AsRef<str>) -> EntitySelect {
        Self::entity(SelectKind::Channel, custom_id.as_ref())
    }
}

impl EntitySelect {
    /// Text shown while nothing is selected.
    #[must_use]
    pub fn placeholder(mut self, text: impl AsRef<str>) -> Self {
        self.menu.placeholder = Some(text.as_ref().into());
        self
    }

    /// Allowed number of selections (default 1..=1).
    #[must_use]
    pub fn values(mut self, min: u8, max: u8) -> Self {
        self.menu.min_values = Some(min);
        self.menu.max_values = Some(max);
        self
    }

    /// Only these channel types (channel selects).
    #[must_use]
    pub fn channel_types(mut self, types: impl IntoIterator<Item = ChannelType>) -> Self {
        self.menu.channel_types = types.into_iter().collect();
        self
    }

    /// Preselect an entity.
    #[must_use]
    pub fn default_value(mut self, id: Id<GenericMarker>, kind: SelectDefaultValueType) -> Self {
        self.menu
            .default_values
            .push(SelectDefaultValue { id, kind });
        self
    }

    /// Show the select greyed out.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.menu.disabled = true;
        self
    }
}

impl From<EntitySelect> for Component {
    fn from(value: EntitySelect) -> Self {
        match value.kind {
            SelectKind::User => Self::UserSelect(value.menu),
            SelectKind::Role => Self::RoleSelect(value.menu),
            SelectKind::Mentionable => Self::MentionableSelect(value.menu),
            SelectKind::Channel => Self::ChannelSelect(value.menu),
        }
    }
}

impl StringSelect {
    /// Selection required (modals).
    #[must_use]
    pub fn required(mut self, required: bool) -> Self {
        self.required = Some(required);
        self
    }
}

impl TextDisplay {
    /// Markdown text (Components V2).
    pub fn new(content: impl AsRef<str>) -> Self {
        Self {
            id: None,
            content: content.as_ref().into(),
        }
    }
}

impl Section {
    /// Up to 3 text displays with an accessory (a button or thumbnail) on
    /// the right (Components V2).
    pub fn new<C: Into<Component>>(
        texts: impl IntoIterator<Item = C>,
        accessory: impl Into<Component>,
    ) -> Self {
        Self {
            id: None,
            components: texts.into_iter().map(Into::into).collect(),
            accessory: Box::new(accessory.into()),
        }
    }
}

impl Thumbnail {
    /// Small image from a URL or `attachment://name` (Components V2).
    pub fn new(url: impl AsRef<str>) -> Self {
        Self {
            id: None,
            media: UnfurledMediaItem::new(url.as_ref()),
            description: None,
            spoiler: false,
        }
    }

    /// Alt text.
    #[must_use]
    pub fn description(mut self, text: impl AsRef<str>) -> Self {
        self.description = Some(text.as_ref().into());
        self
    }

    /// Hide behind a spoiler.
    #[must_use]
    pub fn spoiler(mut self) -> Self {
        self.spoiler = true;
        self
    }
}

impl MediaGallery {
    /// Gallery of 1-10 images or videos (URLs or `attachment://name`).
    pub fn new<S: AsRef<str>>(urls: impl IntoIterator<Item = S>) -> Self {
        Self {
            id: None,
            items: urls
                .into_iter()
                .map(|url| MediaGalleryItem {
                    media: UnfurledMediaItem::new(url.as_ref()),
                    description: None,
                    spoiler: false,
                })
                .collect(),
        }
    }
}

impl FileComponent {
    /// An uploaded file shown in the message: `attachment://name`.
    pub fn new(attachment_url: impl AsRef<str>) -> Self {
        Self {
            id: None,
            file: UnfurledMediaItem::new(attachment_url.as_ref()),
            spoiler: false,
            name: None,
            size: None,
        }
    }

    /// Hide behind a spoiler.
    #[must_use]
    pub fn spoiler(mut self) -> Self {
        self.spoiler = true;
        self
    }
}

impl Separator {
    /// Vertical space with a divider line.
    pub fn new() -> Self {
        Self {
            id: None,
            divider: Some(true),
            spacing: None,
        }
    }

    /// Space without a line.
    pub fn space(spacing: SeparatorSpacing) -> Self {
        Self {
            id: None,
            divider: Some(false),
            spacing: Some(spacing),
        }
    }
}

impl Default for Separator {
    fn default() -> Self {
        Self::new()
    }
}

impl Container {
    /// Visual box around components (Components V2).
    pub fn new<C: Into<Component>>(components: impl IntoIterator<Item = C>) -> Self {
        Self {
            id: None,
            components: components.into_iter().map(Into::into).collect(),
            accent_color: None,
            spoiler: false,
        }
    }

    /// Colored bar on the left (`0xRRGGBB`).
    #[must_use]
    pub fn accent_color(mut self, color: u32) -> Self {
        self.accent_color = Some(color);
        self
    }

    /// Hide behind a spoiler.
    #[must_use]
    pub fn spoiler(mut self) -> Self {
        self.spoiler = true;
        self
    }
}

impl FileUpload {
    /// File upload in a modal (wrap it in a [`Label`]).
    pub fn new(custom_id: impl AsRef<str>) -> Self {
        Self {
            id: None,
            custom_id: custom_id.as_ref().into(),
            min_values: None,
            max_values: None,
            required: None,
            file_types: Vec::new(),
        }
    }

    /// Allowed number of files.
    #[must_use]
    pub fn values(mut self, min: u8, max: u8) -> Self {
        self.min_values = Some(min);
        self.max_values = Some(max);
        self
    }

    /// Allow an empty upload (required by default).
    #[must_use]
    pub fn optional(mut self) -> Self {
        self.required = Some(false);
        self
    }
}

impl ChoiceOption {
    /// Option with a shown label and the value the bot receives.
    pub fn new(label: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        Self {
            value: value.as_ref().into(),
            label: label.as_ref().into(),
            description: None,
            default: false,
        }
    }

    /// Selected by default.
    #[must_use]
    pub fn selected(mut self) -> Self {
        self.default = true;
        self
    }
}

impl RadioGroup {
    /// Single choice in a modal (wrap it in a [`Label`]).
    pub fn new(
        custom_id: impl AsRef<str>,
        options: impl IntoIterator<Item = ChoiceOption>,
    ) -> Self {
        Self {
            id: None,
            custom_id: custom_id.as_ref().into(),
            options: options.into_iter().collect(),
            required: None,
        }
    }
}

impl CheckboxGroup {
    /// Multiple choice in a modal (wrap it in a [`Label`]).
    pub fn new(
        custom_id: impl AsRef<str>,
        options: impl IntoIterator<Item = ChoiceOption>,
    ) -> Self {
        Self {
            id: None,
            custom_id: custom_id.as_ref().into(),
            options: options.into_iter().collect(),
            min_values: None,
            max_values: None,
            required: None,
        }
    }

    /// Allowed number of checked boxes.
    #[must_use]
    pub fn values(mut self, min: u8, max: u8) -> Self {
        self.min_values = Some(min);
        self.max_values = Some(max);
        self
    }
}

impl Checkbox {
    /// Single yes/no checkbox in a modal (wrap it in a [`Label`]).
    pub fn new(custom_id: impl AsRef<str>) -> Self {
        Self {
            id: None,
            custom_id: custom_id.as_ref().into(),
            default: false,
        }
    }

    /// Checked by default.
    #[must_use]
    pub fn checked(mut self) -> Self {
        self.default = true;
        self
    }
}

impl Component {
    /// Whether this is a Components V2 layout/content component (section,
    /// text display, thumbnail, media gallery, file, separator, container).
    /// Messages with these need the `IS_COMPONENTS_V2` flag, which cordex
    /// sets on its own.
    pub const fn is_v2(&self) -> bool {
        matches!(
            self,
            Self::Section(_)
                | Self::TextDisplay(_)
                | Self::Thumbnail(_)
                | Self::MediaGallery(_)
                | Self::File(_)
                | Self::Separator(_)
                | Self::Container(_)
        )
    }
}

/// Whether any top-level component needs the `IS_COMPONENTS_V2` flag.
pub fn needs_components_v2(components: &[Component]) -> bool {
    components.iter().any(Component::is_v2)
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
    fn v2_layout_builds_and_is_detected() {
        let layout: Vec<Component> = vec![
            Container::new([
                Component::from(TextDisplay::new("# Title")),
                Section::new(
                    [TextDisplay::new("Text")],
                    Thumbnail::new("https://x/a.png"),
                )
                .into(),
                Separator::new().into(),
                MediaGallery::new(["https://x/1.png", "https://x/2.png"]).into(),
            ])
            .accent_color(0x0058_65F2)
            .into(),
            ActionRow::new([SelectMenu::channels("ch").channel_types([ChannelType::GuildText])])
                .into(),
        ];
        assert!(needs_components_v2(&layout));
        assert!(!needs_components_v2(&[ActionRow::new([Button::primary(
            "a", "A"
        )])
        .into()]));
        let v = serde_json::to_value(&layout).unwrap();
        assert_eq!(v[0]["type"], 17);
        assert_eq!(v[0]["accent_color"], 0x0058_65F2);
        assert_eq!(v[0]["components"][1]["type"], 9);
        assert_eq!(v[0]["components"][1]["accessory"]["type"], 11);
        assert_eq!(v[1]["components"][0]["type"], 8);
        assert_eq!(v[1]["components"][0]["channel_types"][0], 0);
    }

    #[test]
    fn modal_choices() {
        let radio: Component = Label::new(
            "Pick",
            RadioGroup::new(
                "r",
                [
                    ChoiceOption::new("A", "a").selected(),
                    ChoiceOption::new("B", "b"),
                ],
            ),
        )
        .into();
        let v = serde_json::to_value(&radio).unwrap();
        assert_eq!(v["component"]["type"], 21);
        assert_eq!(v["component"]["options"][0]["default"], true);
        let check: Component = Checkbox::new("c").checked().into();
        assert_eq!(serde_json::to_value(&check).unwrap()["default"], true);
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
