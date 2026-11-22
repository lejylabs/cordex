//! Entitlements (resources/entitlement.mdx).

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::id::marker::{
    ApplicationMarker, EntitlementMarker, GenericMarker, GuildMarker, SkuMarker,
    SubscriptionMarker, UserMarker,
};
use crate::serde_util::int_enum;
use crate::timestamp::Timestamp;

int_enum! {
    /// How the entitlement was granted.
    pub enum EntitlementType: u8 {
        /// Purchased by the user.
        Purchase = 1,
        /// Discord Nitro subscription.
        PremiumSubscription = 2,
        /// Gifted by a developer.
        DeveloperGift = 3,
        /// Purchased in application test mode.
        TestModePurchase = 4,
        /// Granted while the SKU was free.
        FreePurchase = 5,
        /// Gifted by another user.
        UserGift = 6,
        /// Claimed for free as a Nitro subscriber.
        PremiumPurchase = 7,
        /// Purchased as an app subscription.
        ApplicationSubscription = 8,
    }
}

/// Access to a premium SKU for a user or guild.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Entitlement {
    /// Entitlement ID.
    pub id: Id<EntitlementMarker>,
    /// SKU.
    pub sku_id: Id<SkuMarker>,
    /// Parent application.
    pub application_id: Id<ApplicationMarker>,
    /// User granted access.
    pub user_id: Option<Id<UserMarker>>,
    /// Entitlement type.
    #[serde(rename = "type")]
    pub kind: EntitlementType,
    /// Whether the entitlement was deleted.
    #[serde(default)]
    pub deleted: bool,
    /// Start of validity.
    pub starts_at: Option<Timestamp>,
    /// End of validity.
    pub ends_at: Option<Timestamp>,
    /// Guild granted access.
    pub guild_id: Option<Id<GuildMarker>>,
    /// Whether a consumable SKU has been consumed.
    pub consumed: Option<bool>,
    /// Subscription this entitlement belongs to (undocumented on the
    /// structure table, present in the docs example).
    pub subscription_id: Option<Id<SubscriptionMarker>>,
    /// Promotion this entitlement came from (docs example).
    pub promotion_id: Option<Id<GenericMarker>>,
    /// Gift-code flags (docs example).
    pub gift_code_flags: Option<u64>,
}

impl Entitlement {
    /// Whether the entitlement is valid at `now` (not deleted, within
    /// `starts_at`/`ends_at` when those are set).
    pub fn is_active(&self, now: Timestamp) -> bool {
        if self.deleted {
            return false;
        }
        if self.starts_at.is_some_and(|start| now < start) {
            return false;
        }
        if self.ends_at.is_some_and(|end| now >= end) {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_window() {
        let json = r#"{
            "id":"1","sku_id":"2","application_id":"3","type":8,"deleted":false,
            "starts_at":"2022-09-14T17:00:18.704163+00:00",
            "ends_at":"2022-10-14T17:00:18.704163+00:00"
        }"#;
        let ent: Entitlement = serde_json::from_str(json).unwrap();
        let inside = Timestamp::from_millis(1_663_500_000_000);
        let before = Timestamp::from_millis(1_000_000_000_000);
        assert!(ent.is_active(inside));
        assert!(!ent.is_active(before));
        assert_eq!(ent.kind, EntitlementType::ApplicationSubscription);
    }
}
