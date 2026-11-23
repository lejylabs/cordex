//! Typed REST endpoints.
//!
//! Every method documents its route as `` `METHOD /path` ``;
//! `cargo xtask coverage` reads these comments.

/// Endpoint without options: `client.name(args).await`.
macro_rules! simple {
    (
        $(#[$doc:meta])*
        fn $name:ident($($arg:ident: $ty:ty),* $(,)?) -> $out:ty = $method:ident $tpl:literal => $send:ident
    ) => {
        $(#[$doc])*
        pub fn $name(
            &self,
            $($arg: $ty),*
        ) -> impl ::std::future::Future<Output = Result<$out, $crate::error::HttpError>> + Send + 'static {
            let client = self.clone();
            async move {
                client
                    .$method($crate::route::Route::raw($tpl, ($($arg,)*)))
                    .$send()
                    .await
            }
        }
    };
}

/// GET request with optional query parameters, sent with `.await`.
macro_rules! query_request {
    (
        $(#[$doc:meta])*
        $name:ident -> $out:ty {
            $( $(#[$fdoc:meta])* $opt:ident: $oty:ty ),* $(,)?
        }
    ) => {
        $(#[$doc])*
        #[must_use = "send the request with `.await`"]
        pub struct $name {
            client: $crate::client::Client,
            route: $crate::route::Route,
            query: Vec<(&'static str, String)>,
        }

        impl $name {
            #[allow(dead_code, reason = "not every request has options")]
            fn new(client: &$crate::client::Client, route: $crate::route::Route) -> Self {
                Self { client: client.clone(), route, query: Vec::new() }
            }

            $(
                $(#[$fdoc])*
                pub fn $opt(mut self, value: $oty) -> Self {
                    self.query.push((stringify!($opt), value.to_string()));
                    self
                }
            )*
        }

        impl ::std::future::IntoFuture for $name {
            type Output = Result<$out, $crate::error::HttpError>;
            type IntoFuture = ::std::pin::Pin<Box<dyn ::std::future::Future<Output = Self::Output> + Send>>;

            fn into_future(self) -> Self::IntoFuture {
                Box::pin(async move {
                    let mut req = self.client.get(self.route);
                    for (key, value) in &self.query {
                        req = req.query(key, value);
                    }
                    req.send_json().await
                })
            }
        }
    };
}

/// Request with a JSON body built from typed setters, sent with `.await`.
macro_rules! json_request {
    (
        $(#[$doc:meta])*
        $name:ident -> $out:ty => $send:ident {
            $( $(#[$fdoc:meta])* $field:ident: $ty:ty ),* $(,)?
        }
    ) => {
        $(#[$doc])*
        #[must_use = "send the request with `.await`"]
        pub struct $name {
            client: $crate::client::Client,
            method: ::http::Method,
            route: $crate::route::Route,
            body: ::serde_json::Map<String, ::serde_json::Value>,
            reason: Option<String>,
            error: Option<String>,
        }

        impl $name {
            #[allow(dead_code, reason = "not every request is built the same way")]
            fn new(
                client: &$crate::client::Client,
                method: ::http::Method,
                route: $crate::route::Route,
            ) -> Self {
                Self {
                    client: client.clone(),
                    method,
                    route,
                    body: ::serde_json::Map::new(),
                    reason: None,
                    error: None,
                }
            }

            #[allow(dead_code, reason = "used by the generated setters")]
            fn set(mut self, key: &'static str, value: impl ::serde::Serialize) -> Self {
                match ::serde_json::to_value(value) {
                    Ok(value) => {
                        self.body.insert(key.to_owned(), value);
                    }
                    Err(e) => self.error = Some(e.to_string()),
                }
                self
            }

            $(
                $(#[$fdoc])*
                pub fn $field(self, value: $ty) -> Self {
                    self.set(stringify!($field), value)
                }
            )*

            /// Audit-log reason (where Discord supports it).
            pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, $crate::error::HttpError> {
                super::set_reason(&mut self.reason, reason)?;
                Ok(self)
            }
        }

        impl ::std::future::IntoFuture for $name {
            type Output = Result<$out, $crate::error::HttpError>;
            type IntoFuture = ::std::pin::Pin<Box<dyn ::std::future::Future<Output = Self::Output> + Send>>;

            fn into_future(self) -> Self::IntoFuture {
                Box::pin(async move {
                    if let Some(error) = self.error {
                        return Err($crate::error::HttpError::Validation(error.into()));
                    }
                    let mut req = self.client.request(self.method, self.route).json(&self.body)?;
                    if let Some(reason) = &self.reason {
                        req = req.reason(reason)?;
                    }
                    req.$send().await
                })
            }
        }
    };
}

mod app_admin;
mod assets;
mod audit_log;
mod ban;
mod channel;
mod command;
mod entitlement;
mod guild;
mod guild_admin;
mod interaction;
mod invite;
mod member;
mod message;
mod misc;
mod permissions;
mod reaction;
mod role;
mod scheduled_event;
mod thread;
mod user;
mod webhook;

pub use app_admin::{
    AnswerVoters, AutoModerationRuleRequest, CreateStageInstance, GetAnswerVoters,
    GetSkuSubscriptions, UpdateCurrentApplication, UpdateStageInstance,
};
pub use assets::{
    EMOJI_MAX_BYTES, GuildSoundRequest, SOUND_MAX_BYTES, STICKER_MAX_BYTES, UpdateGuildEmoji,
    UpdateGuildSticker,
};
pub use audit_log::GetAuditLog;
pub use command::{CommandBody, CommandPayload};
pub use guild_admin::{
    AddGuildMember, BeginGuildPrune, GetGuildMembers, GetGuildPruneCount, SearchGuildMembers,
    UpdateCurrentMember, UpdateGuild, UpdateGuildWidget, UpdateIncidentActions, UpdateOnboarding,
    UpdateWelcomeScreen,
};
pub use message::GetReactions;
pub use message::{CreateMessage, EditMessage, GetChannelMessages};
pub use misc::{
    AuthorizationInfo, MAX_TARGET_USERS_BULK, SearchGuildMessages, SearchResult, SearchResults,
    TargetUsersJob, UpdateCurrentVoiceState, UpdateGuildTemplate, UpdateVoiceState,
};
pub use permissions::{GetPermissions, PermissionsOnly};
pub use scheduled_event::{GetScheduledEvent, GetScheduledEventUsers, GetScheduledEvents};
pub use thread::{GetArchivedThreads, GetThreadMember, GetThreadMembers, ThreadList};
pub use user::{GatewayUrl, GetCurrentUserGuilds, UpdateCurrentUser};
pub use webhook::{ExecuteWebhook, UpdateWebhook};

use crate::error::HttpError;

pub(crate) fn set_reason(
    slot: &mut Option<String>,
    reason: impl Into<String>,
) -> Result<(), HttpError> {
    let reason = reason.into();
    let _ = crate::encode_audit_log_reason(&reason)?;
    *slot = Some(reason);
    Ok(())
}
