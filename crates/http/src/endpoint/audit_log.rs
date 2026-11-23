//! Audit log.

use cordex_model::audit_log::{AuditLog, AuditLogEvent};
use cordex_model::id::Id;
use cordex_model::id::marker::{AuditLogEntryMarker, GuildMarker, UserMarker};

use crate::client::Client;
use crate::route::Route;

query_request! {
    /// `GET /guilds/{guild_id}/audit-logs`, filters optional.
    GetAuditLog -> AuditLog {
        /// Only entries made by this user.
        user_id: Id<UserMarker>,
        /// Entries before this entry ID.
        before: Id<AuditLogEntryMarker>,
        /// Entries after this entry ID.
        after: Id<AuditLogEntryMarker>,
        /// Number of entries (1–100, default 50).
        limit: u8,
    }
}

impl GetAuditLog {
    /// Only entries of this event type.
    pub fn action_type(mut self, event: AuditLogEvent) -> Self {
        self.query
            .push(("action_type", u16::from(event).to_string()));
        self
    }
}

impl Client {
    /// `GET /guilds/{guild_id}/audit-logs`. Needs `VIEW_AUDIT_LOG`.
    pub fn get_audit_log(&self, guild_id: Id<GuildMarker>) -> GetAuditLog {
        GetAuditLog::new(self, Route::raw("/guilds/{guild_id}/audit-logs", guild_id))
    }
}
