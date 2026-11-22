//! Route identity for REST requests and the rate limiter (PLAN.md §6.2, §7.1).

use std::fmt::{self, Write};

use drs_model::id::Id;
use http::Method;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

/// Unreserved path characters left as-is (RFC 3986).
const PATH_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// Major parameters that Discord uses to split rate-limit buckets.
///
/// `webhook_token` is never printed by [`Debug`].
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct MajorParameters {
    /// `channel_id` path parameter, when present.
    pub channel_id: Option<u64>,
    /// `guild_id` path parameter, when present.
    pub guild_id: Option<u64>,
    /// `webhook_id` path parameter, when present.
    pub webhook_id: Option<u64>,
    webhook_token: Option<Box<str>>,
}

impl MajorParameters {
    /// Webhook token that is part of the bucket key, if any.
    pub fn webhook_token(&self) -> Option<&str> {
        self.webhook_token.as_deref()
    }
}

impl fmt::Debug for MajorParameters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MajorParameters")
            .field("channel_id", &self.channel_id)
            .field("guild_id", &self.guild_id)
            .field("webhook_id", &self.webhook_id)
            .field("webhook_token", &self.webhook_token.as_ref().map(|_| "***"))
            .finish()
    }
}

/// A REST route: HTTP method, path template, filled path and major parameters.
///
/// The rate limiter keys on method + template + [`MajorParameters`]. Typed
/// endpoints and [`Route::raw`] share this type.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Route {
    pub(crate) method: Method,
    pub(crate) template: String,
    pub(crate) path: String,
    pub(crate) major: MajorParameters,
    pub(crate) global_exempt: bool,
}

impl fmt::Debug for Route {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut path = self.path.clone();
        if let Some(token) = self.major.webhook_token() {
            path = path.replace(token, "***");
            let encoded = utf8_percent_encode(token, PATH_SET).to_string();
            if encoded != token {
                path = path.replace(&encoded, "***");
            }
        }
        f.debug_struct("Route")
            .field("method", &self.method)
            .field("template", &self.template)
            .field("path", &path)
            .field("major", &self.major)
            .field("global_exempt", &self.global_exempt)
            .finish()
    }
}

impl Route {
    /// A path template with no parameters, method set later by [`crate::Client::request`].
    pub fn new(template: impl AsRef<str>) -> Self {
        Self::raw(template, ())
    }

    /// Fill `{placeholders}` in `template` from `params`, in order.
    ///
    /// Placeholders named `channel_id`, `guild_id`, `webhook_id` and
    /// `webhook_token` become [`MajorParameters`]. Templates that start with
    /// `/interactions/` are exempt from the global 50 rps limiter.
    ///
    /// # Examples
    ///
    /// ```
    /// use drs_http::Route;
    /// use drs_model::id::{Id, marker::GuildMarker};
    ///
    /// let guild_id = Id::<GuildMarker>::new(1).unwrap();
    /// let route = Route::raw("/guilds/{guild_id}/onboarding", guild_id);
    /// assert_eq!(route.path(), "/guilds/1/onboarding");
    /// assert_eq!(route.major().guild_id, Some(1));
    /// ```
    pub fn raw(template: impl AsRef<str>, params: impl IntoPathParams) -> Self {
        let template = template.as_ref();
        let mut values = Vec::new();
        params.into_path_params(&mut values);
        let (path, major, global_exempt) = substitute(template, &values);
        Self {
            method: Method::GET,
            template: template.to_owned(),
            path,
            major,
            global_exempt,
        }
    }

    /// Marks this route as exempt from the global 50 requests/second limit
    /// (interaction callbacks and interaction webhook follow-ups).
    #[must_use]
    pub fn interaction(mut self) -> Self {
        self.global_exempt = true;
        self
    }

    /// HTTP method. Overwritten by [`crate::Client::request`].
    pub fn method(&self) -> &Method {
        &self.method
    }

    /// Path with parameters filled in (starts with `/`, no host).
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Path template as passed to [`Route::raw`].
    pub fn template(&self) -> &str {
        &self.template
    }

    /// Major parameters extracted from the template.
    pub fn major(&self) -> &MajorParameters {
        &self.major
    }

    /// Whether the global 50 rps limiter is skipped.
    pub fn is_global_exempt(&self) -> bool {
        self.global_exempt
    }
}

/// Values interpolated into a [`Route`] template.
pub trait IntoPathParams {
    /// Append string forms of the path parameters, in placeholder order.
    fn into_path_params(self, out: &mut Vec<String>);
}

impl IntoPathParams for () {
    fn into_path_params(self, _out: &mut Vec<String>) {}
}

impl<T> IntoPathParams for Id<T> {
    fn into_path_params(self, out: &mut Vec<String>) {
        out.push(self.get().to_string());
    }
}

impl IntoPathParams for u64 {
    fn into_path_params(self, out: &mut Vec<String>) {
        out.push(self.to_string());
    }
}

impl IntoPathParams for &str {
    fn into_path_params(self, out: &mut Vec<String>) {
        out.push(self.to_owned());
    }
}

impl IntoPathParams for String {
    fn into_path_params(self, out: &mut Vec<String>) {
        out.push(self);
    }
}

macro_rules! impl_path_tuple {
    ($($A:ident),+) => {
        impl<$($A: ToString),+> IntoPathParams for ($($A,)+) {
            fn into_path_params(self, out: &mut Vec<String>) {
                #[allow(non_snake_case)]
                let ($($A,)+) = self;
                $(out.push($A.to_string());)+
            }
        }
    };
}

impl_path_tuple!(A);
impl_path_tuple!(A, B);
impl_path_tuple!(A, B, C);
impl_path_tuple!(A, B, C, D);
impl_path_tuple!(A, B, C, D, E);

fn substitute(template: &str, params: &[String]) -> (String, MajorParameters, bool) {
    let mut path = String::with_capacity(template.len());
    let mut major = MajorParameters::default();
    let mut param_idx = 0;
    let mut rest = template;
    let global_exempt = template.starts_with("/interactions/");

    while let Some(start) = rest.find('{') {
        let Some(rel_end) = rest[start + 1..].find('}') else {
            path.push_str(rest);
            rest = "";
            break;
        };
        let end = start + 1 + rel_end;
        path.push_str(&rest[..start]);
        let name = &rest[start + 1..end];
        if param_idx < params.len() {
            let raw = &params[param_idx];
            param_idx += 1;
            match name {
                "channel_id" => major.channel_id = raw.parse().ok(),
                "guild_id" => major.guild_id = raw.parse().ok(),
                "webhook_id" => major.webhook_id = raw.parse().ok(),
                "webhook_token" => major.webhook_token = Some(raw.clone().into_boxed_str()),
                _ => {}
            }
            let _ = write!(path, "{}", utf8_percent_encode(raw, PATH_SET));
        } else {
            path.push('{');
            path.push_str(name);
            path.push('}');
        }
        rest = &rest[end + 1..];
    }
    path.push_str(rest);
    (path, major, global_exempt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use drs_model::id::marker::{ChannelMarker, GuildMarker};

    #[test]
    fn fills_major_parameters() {
        let guild = Id::<GuildMarker>::new(42).unwrap();
        let channel = Id::<ChannelMarker>::new(7).unwrap();
        let route = Route::raw("/guilds/{guild_id}/channels/{channel_id}", (guild, channel));
        assert_eq!(route.path(), "/guilds/42/channels/7");
        assert_eq!(route.major.guild_id, Some(42));
        assert_eq!(route.major.channel_id, Some(7));
        assert!(!route.global_exempt);
    }

    #[test]
    fn interaction_templates_are_global_exempt() {
        let route = Route::raw(
            "/interactions/{interaction_id}/{interaction_token}/callback",
            (1u64, "tok"),
        );
        assert!(route.global_exempt);
        assert_eq!(route.path(), "/interactions/1/tok/callback");
    }

    #[test]
    fn webhook_token_redacted_in_debug() {
        let route = Route::raw(
            "/webhooks/{webhook_id}/{webhook_token}",
            (9u64, "super-secret"),
        );
        let debug = format!("{route:?}");
        assert!(!debug.contains("super-secret"));
        assert_eq!(route.major.webhook_token(), Some("super-secret"));
    }

    #[test]
    fn encodes_unsafe_path_bytes() {
        let route = Route::raw("/guilds/{guild_id}/foo bar", 1u64);
        assert_eq!(route.path(), "/guilds/1/foo bar");
        let route = Route::raw("/webhooks/{webhook_id}/{webhook_token}", (1u64, "a b"));
        assert_eq!(route.path(), "/webhooks/1/a%20b");
    }
}
