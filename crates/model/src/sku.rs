//! SKUs and subscriptions (resources/sku.mdx, resources/subscription.mdx).

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::id::marker::{
    ApplicationMarker, EntitlementMarker, SkuMarker, SubscriptionMarker, UserMarker,
};
use crate::serde_util::int_enum;
use crate::timestamp::Timestamp;

/// A premium offering of an application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Sku {
    /// SKU ID.
    pub id: Id<SkuMarker>,
    /// Kind of SKU.
    #[serde(rename = "type")]
    pub kind: SkuType,
    /// Application that owns it.
    pub application_id: Id<ApplicationMarker>,
    /// Customer-facing name.
    pub name: Box<str>,
    /// URL slug.
    pub slug: Box<str>,
    /// SKU flags (bit 2 available, bit 7 guild subscription, bit 8 user
    /// subscription).
    #[serde(default)]
    pub flags: u64,
}

int_enum! {
    /// SKU type.
    pub enum SkuType: u8 {
        /// One-time purchase, owned forever.
        Durable = 2,
        /// One-time purchase that can be consumed.
        Consumable = 3,
        /// Recurring subscription.
        Subscription = 5,
        /// System-generated group of a subscription SKU.
        SubscriptionGroup = 6,
    }
}

/// A user's subscription to an SKU.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Subscription {
    /// Subscription ID.
    pub id: Id<SubscriptionMarker>,
    /// Subscribed user.
    pub user_id: Id<UserMarker>,
    /// Subscribed SKUs.
    #[serde(default)]
    pub sku_ids: Vec<Id<SkuMarker>>,
    /// Granted entitlements.
    #[serde(default)]
    pub entitlement_ids: Vec<Id<EntitlementMarker>>,
    /// SKUs the subscription renews into.
    #[serde(default)]
    pub renewal_sku_ids: Option<Vec<Id<SkuMarker>>>,
    /// Start of the current period.
    pub current_period_start: Timestamp,
    /// End of the current period.
    pub current_period_end: Timestamp,
    /// Status.
    pub status: SubscriptionStatus,
    /// When it was canceled.
    pub canceled_at: Option<Timestamp>,
    /// ISO 3166-1 alpha-2 country code (only with the private OAuth scope).
    pub country: Option<Box<str>>,
}

int_enum! {
    /// Subscription status.
    pub enum SubscriptionStatus: u8 {
        /// Active and renews.
        Active = 0,
        /// Active, does not renew.
        Ending = 1,
        /// Ended.
        Inactive = 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sku_and_subscription_parse() {
        let sku: Sku = serde_json::from_str(
            r#"{"id":"1","type":5,"application_id":"2","name":"Pro","slug":"pro","flags":128}"#,
        )
        .unwrap();
        assert_eq!(sku.kind, SkuType::Subscription);
        let sub: Subscription = serde_json::from_str(
            r#"{"id":"3","user_id":"4","sku_ids":["1"],"entitlement_ids":[],"renewal_sku_ids":null,
            "current_period_start":"2026-11-01T00:00:00+00:00","current_period_end":"2026-12-01T00:00:00+00:00",
            "status":1,"canceled_at":null}"#,
        )
        .unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Ending);
    }
}
