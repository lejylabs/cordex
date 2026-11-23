//! Message and modal components (components/reference.mdx), including
//! Components V2 (message flag `IS_COMPONENTS_V2`).
//!
//! [`Component`] dispatches on the integer `type`. Unknown or undocumented
//! types (e.g. type 16) are kept as raw JSON in [`Component::Unknown`] and
//! serialized back unchanged.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::value::RawValue;

use crate::channel::ChannelType;
use crate::emoji::Emoji;
use crate::id::Id;
use crate::id::marker::{AttachmentMarker, GenericMarker, SkuMarker};
mod builders;

pub use builders::{EntitySelect, SelectKind, needs_components_v2};

use crate::serde_util::{bitflags_serde_int, int_enum, str_enum};

/// Any component.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Component {
    /// 1: row of interactive components.
    ActionRow(ActionRow),
    /// 2: button.
    Button(Button),
    /// 3: select with predefined text options.
    StringSelect(StringSelect),
    /// 4: text input (modals).
    TextInput(TextInput),
    /// 5: user select.
    UserSelect(SelectMenu),
    /// 6: role select.
    RoleSelect(SelectMenu),
    /// 7: user and role select.
    MentionableSelect(SelectMenu),
    /// 8: channel select.
    ChannelSelect(SelectMenu),
    /// 9: text with an accessory.
    Section(Section),
    /// 10: markdown text.
    TextDisplay(TextDisplay),
    /// 11: small image accessory.
    Thumbnail(Thumbnail),
    /// 12: image gallery.
    MediaGallery(MediaGallery),
    /// 13: attached file.
    File(FileComponent),
    /// 14: vertical spacing.
    Separator(Separator),
    /// 17: visual group.
    Container(Container),
    /// 18: label + description around one component (modals).
    Label(Label),
    /// 19: file upload (modals).
    FileUpload(FileUpload),
    /// 21: single choice (modals).
    RadioGroup(RadioGroup),
    /// 22: multiple choice (modals).
    CheckboxGroup(CheckboxGroup),
    /// 23: yes/no checkbox (modals).
    Checkbox(Checkbox),
    /// A type this version does not know (or an undocumented one).
    Unknown(UnknownComponent),
}

/// Raw form of an unknown component.
#[derive(Debug, Clone)]
pub struct UnknownComponent {
    /// The `type` value.
    pub kind: u8,
    /// The whole component as raw JSON.
    pub raw: Box<RawValue>,
}

impl PartialEq for UnknownComponent {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.raw.get() == other.raw.get()
    }
}

impl Eq for UnknownComponent {}

impl Component {
    /// The integer `type`.
    pub const fn kind(&self) -> u8 {
        match self {
            Self::ActionRow(_) => 1,
            Self::Button(_) => 2,
            Self::StringSelect(_) => 3,
            Self::TextInput(_) => 4,
            Self::UserSelect(_) => 5,
            Self::RoleSelect(_) => 6,
            Self::MentionableSelect(_) => 7,
            Self::ChannelSelect(_) => 8,
            Self::Section(_) => 9,
            Self::TextDisplay(_) => 10,
            Self::Thumbnail(_) => 11,
            Self::MediaGallery(_) => 12,
            Self::File(_) => 13,
            Self::Separator(_) => 14,
            Self::Container(_) => 17,
            Self::Label(_) => 18,
            Self::FileUpload(_) => 19,
            Self::RadioGroup(_) => 21,
            Self::CheckboxGroup(_) => 22,
            Self::Checkbox(_) => 23,
            Self::Unknown(u) => u.kind,
        }
    }
}

impl<'de> Deserialize<'de> for Component {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Kind {
            #[serde(rename = "type")]
            kind: u8,
        }

        // Keep the component raw, peek `"type"` at top-level without a full
        // first-pass parse (falling back to serde for escaped/unusual JSON),
        // then parse the matching struct.
        let raw = Box::<RawValue>::deserialize(deserializer)?;
        let json = raw.get();
        let kind = match peek_component_type(json) {
            Some(k) => k,
            None => {
                serde_json::from_str::<Kind>(json)
                    .map_err(D::Error::custom)?
                    .kind
            }
        };
        let parsed = match kind {
            1 => serde_json::from_str(json).map(Self::ActionRow),
            2 => serde_json::from_str(json).map(Self::Button),
            3 => serde_json::from_str(json).map(Self::StringSelect),
            4 => serde_json::from_str(json).map(Self::TextInput),
            5 => serde_json::from_str(json).map(Self::UserSelect),
            6 => serde_json::from_str(json).map(Self::RoleSelect),
            7 => serde_json::from_str(json).map(Self::MentionableSelect),
            8 => serde_json::from_str(json).map(Self::ChannelSelect),
            9 => serde_json::from_str(json).map(Self::Section),
            10 => serde_json::from_str(json).map(Self::TextDisplay),
            11 => serde_json::from_str(json).map(Self::Thumbnail),
            12 => serde_json::from_str(json).map(Self::MediaGallery),
            13 => serde_json::from_str(json).map(Self::File),
            14 => serde_json::from_str(json).map(Self::Separator),
            17 => serde_json::from_str(json).map(Self::Container),
            18 => serde_json::from_str(json).map(Self::Label),
            19 => serde_json::from_str(json).map(Self::FileUpload),
            21 => serde_json::from_str(json).map(Self::RadioGroup),
            22 => serde_json::from_str(json).map(Self::CheckboxGroup),
            23 => serde_json::from_str(json).map(Self::Checkbox),
            _ => return Ok(Self::Unknown(UnknownComponent { kind, raw })),
        };
        parsed.map_err(D::Error::custom)
    }
}

/// Fast top-level scan for `"type": <u8>` in a JSON object so nested
/// components (containers, action rows, sections) skip a full first-pass parse.
fn peek_component_type(json: &str) -> Option<u8> {
    let b = json.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    if b.get(i) != Some(&b'{') {
        return None;
    }
    i += 1;
    let mut depth: u32 = 1;
    while i < b.len() {
        match b[i] {
            b'"' => {
                let key_start = i + 1;
                i += 1;
                while i < b.len() {
                    if b[i] == b'\\' {
                        i += 2;
                    } else if b[i] == b'"' {
                        break;
                    } else {
                        i += 1;
                    }
                }
                let key_end = i;
                i += 1;
                if depth == 1 && &b[key_start..key_end.min(b.len())] == b"type" {
                    while i < b.len() && b[i].is_ascii_whitespace() {
                        i += 1;
                    }
                    if b.get(i) == Some(&b':') {
                        i += 1;
                        while i < b.len() && b[i].is_ascii_whitespace() {
                            i += 1;
                        }
                        let num_start = i;
                        while i < b.len() && b[i].is_ascii_digit() {
                            i += 1;
                        }
                        if i > num_start {
                            return std::str::from_utf8(&b[num_start..i]).ok()?.parse().ok();
                        }
                        return None;
                    }
                }
            }
            b'{' | b'[' => {
                depth += 1;
                i += 1;
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

impl Serialize for Component {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        /// Adds `"type"` in front of the component's own fields.
        #[derive(Serialize)]
        struct Tagged<'a, T: Serialize> {
            #[serde(rename = "type")]
            kind: u8,
            #[serde(flatten)]
            inner: &'a T,
        }

        fn tagged<S: Serializer, T: Serialize>(
            kind: u8,
            inner: &T,
            s: S,
        ) -> Result<S::Ok, S::Error> {
            Tagged { kind, inner }.serialize(s)
        }

        let kind = self.kind();
        match self {
            Self::ActionRow(c) => tagged(kind, c, serializer),
            Self::Button(c) => tagged(kind, c, serializer),
            Self::StringSelect(c) => tagged(kind, c, serializer),
            Self::TextInput(c) => tagged(kind, c, serializer),
            Self::UserSelect(c)
            | Self::RoleSelect(c)
            | Self::MentionableSelect(c)
            | Self::ChannelSelect(c) => tagged(kind, c, serializer),
            Self::Section(c) => tagged(kind, c, serializer),
            Self::TextDisplay(c) => tagged(kind, c, serializer),
            Self::Thumbnail(c) => tagged(kind, c, serializer),
            Self::MediaGallery(c) => tagged(kind, c, serializer),
            Self::File(c) => tagged(kind, c, serializer),
            Self::Separator(c) => tagged(kind, c, serializer),
            Self::Container(c) => tagged(kind, c, serializer),
            Self::Label(c) => tagged(kind, c, serializer),
            Self::FileUpload(c) => tagged(kind, c, serializer),
            Self::RadioGroup(c) => tagged(kind, c, serializer),
            Self::CheckboxGroup(c) => tagged(kind, c, serializer),
            Self::Checkbox(c) => tagged(kind, c, serializer),
            Self::Unknown(u) => u.raw.serialize(serializer),
        }
    }
}

/// Fields shared by all components: the optional numeric `id`.
macro_rules! component_struct {
    (
        $(#[$meta:meta])*
        pub struct $name:ident {
            $( $(#[$fmeta:meta])* pub $field:ident: $ty:ty, )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[non_exhaustive]
        pub struct $name {
            /// Optional identifier unique within the message (set by Discord if omitted).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub id: Option<u32>,
            $( $(#[$fmeta])* pub $field: $ty, )*
        }
    };
}

component_struct! {
    /// Action row: up to 5 buttons or one select.
    pub struct ActionRow {
        /// Child components.
        pub components: Vec<Component>,
    }
}

component_struct! {
    /// Button.
    pub struct Button {
        /// Style.
        pub style: ButtonStyle,
        /// Text (max. 80 characters).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub label: Option<Box<str>>,
        /// Emoji (`id`, `name`, `animated`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub emoji: Option<Emoji>,
        /// Developer ID (max. 100 characters); not for link and premium buttons.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub custom_id: Option<Box<str>>,
        /// SKU of a premium button.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sku_id: Option<Id<SkuMarker>>,
        /// URL of a link button.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub url: Option<Box<str>>,
        /// Whether the button is disabled.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub disabled: bool,
    }
}

int_enum! {
    /// Button style.
    pub enum ButtonStyle: u8 {
        /// Blurple.
        Primary = 1,
        /// Grey.
        Secondary = 2,
        /// Green.
        Success = 3,
        /// Red.
        Danger = 4,
        /// Navigates to a URL.
        Link = 5,
        /// Purchase button for an SKU.
        Premium = 6,
    }
}

component_struct! {
    /// String select.
    pub struct StringSelect {
        /// Developer ID.
        pub custom_id: Box<str>,
        /// Options (max. 25).
        pub options: Vec<SelectOption>,
        /// Placeholder text.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub placeholder: Option<Box<str>>,
        /// Minimum selections (0–25, default 1).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub min_values: Option<u8>,
        /// Maximum selections (1–25, default 1).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub max_values: Option<u8>,
        /// Whether a selection is required (modals).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub required: Option<bool>,
        /// Whether the select is disabled (messages).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub disabled: bool,
    }
}

/// Option of a string select.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SelectOption {
    /// Shown text (max. 100 characters).
    pub label: Box<str>,
    /// Developer value (max. 100 characters).
    pub value: Box<str>,
    /// Additional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Box<str>>,
    /// Emoji.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<Emoji>,
    /// Selected by default.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub default: bool,
}

component_struct! {
    /// Text input in a modal.
    pub struct TextInput {
        /// Developer ID.
        pub custom_id: Box<str>,
        /// Style.
        pub style: TextInputStyle,
        /// Minimum length (0–4000).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub min_length: Option<u16>,
        /// Maximum length (1–4000).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub max_length: Option<u16>,
        /// Whether input is required (default true).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub required: Option<bool>,
        /// Prefilled value.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<Box<str>>,
        /// Placeholder text.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub placeholder: Option<Box<str>>,
    }
}

int_enum! {
    /// Text input style.
    pub enum TextInputStyle: u8 {
        /// Single line.
        Short = 1,
        /// Multiple lines.
        Paragraph = 2,
    }
}

component_struct! {
    /// User, role, mentionable or channel select (the `type` decides which).
    pub struct SelectMenu {
        /// Developer ID.
        pub custom_id: Box<str>,
        /// Allowed channel types (channel select only).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub channel_types: Vec<ChannelType>,
        /// Placeholder text.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub placeholder: Option<Box<str>>,
        /// Preselected values.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub default_values: Vec<SelectDefaultValue>,
        /// Minimum selections.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub min_values: Option<u8>,
        /// Maximum selections.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub max_values: Option<u8>,
        /// Whether a selection is required (modals).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub required: Option<bool>,
        /// Whether the select is disabled (messages).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub disabled: bool,
    }
}

/// Preselected value of an auto-populated select.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SelectDefaultValue {
    /// User, role or channel ID.
    pub id: Id<GenericMarker>,
    /// Kind of the ID.
    #[serde(rename = "type")]
    pub kind: SelectDefaultValueType,
}

str_enum! {
    /// Kind of a select default value.
    pub enum SelectDefaultValueType {
        /// User.
        User = "user",
        /// Role.
        Role = "role",
        /// Channel.
        Channel = "channel",
    }
}

component_struct! {
    /// Section: 1–3 text displays with an accessory.
    pub struct Section {
        /// Text displays.
        pub components: Vec<Component>,
        /// Thumbnail or button.
        pub accessory: Box<Component>,
    }
}

component_struct! {
    /// Markdown text.
    pub struct TextDisplay {
        /// Markdown content.
        pub content: Box<str>,
    }
}

component_struct! {
    /// Small image, used as section accessory.
    pub struct Thumbnail {
        /// Image (URL or `attachment://name`).
        pub media: UnfurledMediaItem,
        /// Alt text (max. 1024 characters).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<Box<str>>,
        /// Blurred as spoiler.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub spoiler: bool,
    }
}

component_struct! {
    /// Gallery of 1–10 media items.
    pub struct MediaGallery {
        /// Items.
        pub items: Vec<MediaGalleryItem>,
    }
}

/// Item of a media gallery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MediaGalleryItem {
    /// Image or video.
    pub media: UnfurledMediaItem,
    /// Alt text (max. 1024 characters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Box<str>>,
    /// Blurred as spoiler.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub spoiler: bool,
}

component_struct! {
    /// Attached file (`attachment://name` only).
    pub struct FileComponent {
        /// The file.
        pub file: UnfurledMediaItem,
        /// Blurred as spoiler.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub spoiler: bool,
        /// File name (set by Discord).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<Box<str>>,
        /// Size in bytes (set by Discord).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub size: Option<u64>,
    }
}

component_struct! {
    /// Vertical padding, optionally with a line.
    pub struct Separator {
        /// Whether a divider line is shown (default true).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub divider: Option<bool>,
        /// Padding size.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub spacing: Option<SeparatorSpacing>,
    }
}

int_enum! {
    /// Separator padding.
    pub enum SeparatorSpacing: u8 {
        /// Small padding.
        Small = 1,
        /// Large padding.
        Large = 2,
    }
}

component_struct! {
    /// Visual group with an optional accent color.
    pub struct Container {
        /// Child components.
        pub components: Vec<Component>,
        /// Accent color (RGB).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub accent_color: Option<u32>,
        /// Blurred as spoiler.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub spoiler: bool,
    }
}

component_struct! {
    /// Label and description around one modal component.
    pub struct Label {
        /// Label (max. 45 characters).
        pub label: Box<str>,
        /// Description (max. 100 characters).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<Box<str>>,
        /// The wrapped component.
        pub component: Box<Component>,
    }
}

component_struct! {
    /// File upload in a modal.
    pub struct FileUpload {
        /// Developer ID.
        pub custom_id: Box<str>,
        /// Minimum files (0–10).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub min_values: Option<u8>,
        /// Maximum files (1–10).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub max_values: Option<u8>,
        /// Whether an upload is required.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub required: Option<bool>,
        /// Allowed file types.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub file_types: Vec<Box<str>>,
    }
}

component_struct! {
    /// Single-choice group in a modal.
    pub struct RadioGroup {
        /// Developer ID.
        pub custom_id: Box<str>,
        /// Options.
        pub options: Vec<ChoiceOption>,
        /// Whether a choice is required.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub required: Option<bool>,
    }
}

component_struct! {
    /// Multiple-choice group in a modal.
    pub struct CheckboxGroup {
        /// Developer ID.
        pub custom_id: Box<str>,
        /// Options.
        pub options: Vec<ChoiceOption>,
        /// Minimum selections.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub min_values: Option<u8>,
        /// Maximum selections.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub max_values: Option<u8>,
        /// Whether a selection is required.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub required: Option<bool>,
    }
}

/// Option of a radio or checkbox group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ChoiceOption {
    /// Developer value.
    pub value: Box<str>,
    /// Shown text.
    pub label: Box<str>,
    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Box<str>>,
    /// Selected by default.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub default: bool,
}

component_struct! {
    /// Single yes/no checkbox in a modal.
    pub struct Checkbox {
        /// Developer ID.
        pub custom_id: Box<str>,
        /// Checked by default.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub default: bool,
    }
}

/// Media reference: a URL or `attachment://name`; Discord fills the rest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UnfurledMediaItem {
    /// URL or `attachment://name`.
    pub url: Box<str>,
    /// Proxied URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_url: Option<Box<str>>,
    /// Height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// Width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Thumbhash placeholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<Box<str>>,
    /// Placeholder version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder_version: Option<u8>,
    /// Media type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<Box<str>>,
    /// Flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<UnfurledMediaItemFlags>,
    /// Content scan result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_scan_metadata: Option<ContentScanMetadata>,
    /// Attachment of an `attachment://` reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachment_id: Option<Id<AttachmentMarker>>,
}

impl UnfurledMediaItem {
    /// Media item from a URL or `attachment://name`.
    pub fn new(url: impl Into<Box<str>>) -> Self {
        Self {
            url: url.into(),
            proxy_url: None,
            height: None,
            width: None,
            placeholder: None,
            placeholder_version: None,
            content_type: None,
            flags: None,
            content_scan_metadata: None,
            attachment_id: None,
        }
    }
}

bitflags::bitflags! {
    /// Unfurled media item flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct UnfurledMediaItemFlags: u64 {
        /// Animated image.
        const IS_ANIMATED = 1 << 0;
    }
}

bitflags_serde_int!(UnfurledMediaItemFlags, u64);

/// Content scan result of a media item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ContentScanMetadata {
    /// Scan flags.
    pub flags: ContentScanFlags,
}

bitflags::bitflags! {
    /// Content scan flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct ContentScanFlags: u64 {
        /// Flagged as explicit.
        const IS_EXPLICIT_CONTENT = 1 << 0;
        /// Flagged as gore.
        const IS_GORE_CONTENT = 1 << 1;
        /// Flagged as self-harm.
        const IS_SELF_HARM_CONTENT = 1 << 2;
    }
}

bitflags_serde_int!(ContentScanFlags, u64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_row_with_button_round_trip() {
        let json = r#"{"type":1,"components":[{"type":2,"style":1,"label":"Click me!","custom_id":"click_one"}]}"#;
        let c: Component = serde_json::from_str(json).unwrap();
        let Component::ActionRow(row) = &c else {
            panic!("expected action row")
        };
        assert!(
            matches!(&row.components[0], Component::Button(b) if b.style == ButtonStyle::Primary)
        );
        assert_eq!(serde_json::to_string(&c).unwrap(), json);
    }

    #[test]
    fn components_v2_container() {
        let json = r#"{"type":17,"accent_color":703487,"components":[
            {"type":10,"content":"hi"},
            {"type":14,"divider":true,"spacing":1},
            {"type":9,"components":[{"type":10,"content":"x"}],
             "accessory":{"type":11,"media":{"url":"https://example.com/a.png"}}},
            {"type":12,"items":[{"media":{"url":"attachment://b.png"},"spoiler":true}]},
            {"type":13,"file":{"url":"attachment://c.txt"}}
        ]}"#;
        let c: Component = serde_json::from_str(json).unwrap();
        let again: Component = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(c, again);
        let Component::Container(container) = c else {
            panic!("expected container")
        };
        assert_eq!(container.accent_color, Some(703_487));
        assert_eq!(container.components.len(), 5);
    }

    #[test]
    fn modal_components() {
        let json = r#"{"type":18,"label":"Pick","component":
            {"type":21,"custom_id":"r","options":[{"value":"a","label":"A","default":true}]}}"#;
        let c: Component = serde_json::from_str(json).unwrap();
        assert!(
            matches!(c, Component::Label(ref l) if matches!(*l.component, Component::RadioGroup(_)))
        );
        let again: Component = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(c, again);
    }

    #[test]
    fn undocumented_type_is_kept_raw() {
        let json = r#"{"type":16,"some":"data","n":[1,2]}"#;
        let c: Component = serde_json::from_str(json).unwrap();
        assert_eq!(c.kind(), 16);
        assert!(matches!(c, Component::Unknown(_)));
        assert_eq!(serde_json::to_string(&c).unwrap(), json);
    }

    #[test]
    fn invalid_known_component_is_an_error() {
        assert!(serde_json::from_str::<Component>(r#"{"type":2}"#).is_err());
        assert!(serde_json::from_str::<Component>(r#"{"no_type":1}"#).is_err());
    }
}
