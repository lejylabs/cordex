//! Application identity profiles (resources/application-identity-profile.mdx).

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::id::marker::UserMarker;
use crate::serde_util::int_enum;

/// External account identity for an application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ApplicationIdentity {
    /// Discord user this identity is linked to (list endpoints).
    #[serde(default)]
    pub user_id: Option<Id<UserMarker>>,
    /// External account provider type.
    pub provider_type: Box<str>,
    /// Provider-specific identifier; omitted when empty.
    #[serde(default)]
    pub provider_id: Option<Box<str>>,
    /// User ID in the external system.
    pub provider_issued_user_id: Box<str>,
}

/// Wrapped list from identity GET endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ApplicationIdentities {
    /// Matching identities.
    #[serde(default)]
    pub identities: Vec<ApplicationIdentity>,
}

/// Game stats stored on an application identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ApplicationIdentityProfile {
    /// Username in the external system.
    pub username: Option<Box<str>>,
    /// Application-defined data Discord does not consume.
    pub metadata: Option<serde_json::Value>,
    /// Game stats.
    pub data: Option<ProfileData>,
}

/// Profile payload (`primary` + `dynamic`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ProfileData {
    /// Pre-configured stats.
    #[serde(default)]
    pub primary: Option<PrimaryProfileData>,
    /// Custom stats.
    #[serde(default)]
    pub dynamic: Vec<DynamicField>,
}

/// Pre-configured game stat fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[non_exhaustive]
#[serde(default)]
pub struct PrimaryProfileData {
    /// Current season name.
    pub season: Option<Box<str>>,
    /// Current rank name.
    pub rank_name: Option<Box<str>>,
    /// Image for the current rank.
    pub rank_image: Option<Media>,
    /// Highest rank achieved.
    pub highest_rank: Option<Box<str>>,
    /// Image for the highest rank.
    pub highest_rank_image: Option<Media>,
    /// Featured character name.
    pub featured_played_character: Option<Box<str>>,
    /// Featured character image.
    pub featured_played_character_image: Option<Media>,
    /// Total playtime in hours.
    pub playtime_hours: Option<f64>,
    /// Total wins.
    pub total_wins: Option<u64>,
    /// Wins in the current period.
    pub current_period_wins: Option<u64>,
    /// Total games played.
    pub total_games: Option<u64>,
    /// Games in the current period.
    pub current_period_games: Option<u64>,
    /// Total kills.
    pub total_kills: Option<u64>,
    /// Kills in the current period.
    pub current_period_kills: Option<u64>,
    /// Total assists.
    pub total_assists: Option<u64>,
    /// Assists in the current period.
    pub current_period_assists: Option<u64>,
    /// Total deaths.
    pub total_deaths: Option<u64>,
    /// Deaths in the current period.
    pub current_period_deaths: Option<u64>,
}

int_enum! {
    /// Dynamic profile field type.
    pub enum DynamicFieldType: u8 {
        /// Text value.
        String = 1,
        /// Numeric value.
        Number = 2,
        /// Media object.
        Media = 3,
    }
}

/// Custom game stat field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum DynamicField {
    /// `type` 1.
    String(DynamicStringField),
    /// `type` 2.
    Number(DynamicNumberField),
    /// `type` 3.
    Media(DynamicMediaField),
}

/// Dynamic string field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DynamicStringField {
    /// Always 1.
    #[serde(rename = "type")]
    pub kind: DynamicFieldType,
    /// Data key.
    pub name: Box<str>,
    /// Text value.
    pub value: Box<str>,
}

/// Dynamic number field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DynamicNumberField {
    /// Always 2.
    #[serde(rename = "type")]
    pub kind: DynamicFieldType,
    /// Data key.
    pub name: Box<str>,
    /// Numeric value.
    pub value: f64,
}

/// Dynamic media field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DynamicMediaField {
    /// Always 3.
    #[serde(rename = "type")]
    pub kind: DynamicFieldType,
    /// Data key.
    pub name: Box<str>,
    /// Media.
    pub value: Media,
}

/// Media asset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Media {
    /// Public URL.
    pub url: Box<str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_profile_from_docs() {
        let json = r#"{
            "username": "johndoe123",
            "metadata": null,
            "data": {
                "primary": {
                    "season": "Season 3",
                    "rank_name": "Silver",
                    "rank_image": {"url": "https://example.com/assets/rank-images/silver.png"},
                    "highest_rank": "Platinum",
                    "highest_rank_image": {"url": "https://example.com/assets/rank-images/platinum.png"},
                    "featured_played_character": "John Doe",
                    "featured_played_character_image": {"url": "https://example.com/assets/character-images/john-doe.png"},
                    "playtime_hours": 69.41,
                    "total_wins": 57,
                    "current_period_wins": 8,
                    "total_games": 100,
                    "current_period_games": 10,
                    "total_kills": 253,
                    "current_period_kills": 35,
                    "total_assists": 478,
                    "current_period_assists": 68,
                    "total_deaths": 561,
                    "current_period_deaths": 21
                },
                "dynamic": [
                    {"type": 1, "name": "my_string", "value": "hello"},
                    {"type": 2, "name": "my_number", "value": 123.45},
                    {"type": 3, "name": "my_media", "value": {"url": "https://example.com/some-media.png"}}
                ]
            }
        }"#;
        let profile: ApplicationIdentityProfile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.username.as_deref(), Some("johndoe123"));
        let data = profile.data.unwrap();
        assert_eq!(data.primary.unwrap().total_wins, Some(57));
        assert_eq!(data.dynamic.len(), 3);
        assert!(matches!(data.dynamic[0], DynamicField::String(_)));
        assert!(matches!(data.dynamic[1], DynamicField::Number(_)));
        assert!(matches!(data.dynamic[2], DynamicField::Media(_)));
    }
}
