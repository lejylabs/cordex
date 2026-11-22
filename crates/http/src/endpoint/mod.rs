//! Typed P0 REST endpoints (PLAN.md §6.2).

mod ban;
mod channel;
mod command;
mod entitlement;
mod guild;
mod interaction;
mod invite;
mod member;
mod message;
mod reaction;
mod role;
mod scheduled_event;
mod thread;
mod user;
mod webhook;

pub use command::{CommandBody, CommandPayload};
pub use message::{CreateMessage, EditMessage, GetChannelMessages};
pub use webhook::ExecuteWebhook;

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
