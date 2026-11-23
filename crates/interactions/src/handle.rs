//! Responding to one interaction.
//!
//! [`InteractionHandle`] hides the callback types and the 3-second rule:
//!
//! - [`respond`](InteractionHandle::respond) sends the initial message, or
//!   edits the deferred response if the interaction was already deferred
//!   (for example by [`auto_defer`](InteractionHandle::auto_defer)).
//! - A second initial response returns [`InteractionError::AlreadyResponded`]
//!   instead of a Discord 400.
//! - Edits and follow-ups check the 15-minute token lifetime locally.

use std::sync::Arc;
use std::time::{Duration, Instant};

use cordex_http::{CONTENT_LIMIT, Client, EMBED_COUNT_LIMIT};
use cordex_model::command::CommandOptionChoice;
use cordex_model::component::Component;
use cordex_model::id::Id;
use cordex_model::id::marker::{ApplicationMarker, InteractionMarker};
use cordex_model::interaction::{
    Interaction, InteractionCallbackData, InteractionCallbackType, InteractionResponse,
    InteractionType,
};
use cordex_model::message::Message;
use cordex_model::token::InteractionToken;
use tokio::sync::Mutex;

use crate::command::Choice;
use crate::error::InteractionError;
use crate::modal::Modal;
use crate::reply::Reply;

/// Time Discord gives for the initial response.
pub const INITIAL_RESPONSE_WINDOW: Duration = Duration::from_secs(3);
/// Lifetime of the interaction token for edits and follow-ups.
pub const TOKEN_LIFETIME: Duration = Duration::from_mins(15);
/// Default delay for [`InteractionHandle::auto_defer`]: leaves 500 ms for
/// the defer request itself.
pub const DEFAULT_AUTO_DEFER: Duration = Duration::from_millis(2500);

/// Copy the parts of a [`Reply`] both REST builders support.
macro_rules! apply_reply {
    ($req:ident, $reply:ident) => {
        if let Some(content) = $reply.content.clone() {
            $req = $req.content(content)?;
        }
        for embed in $reply.embeds.iter().cloned() {
            $req = $req.embed(embed)?;
        }
        if let Some(components) = $reply.components.clone() {
            $req = $req.components(components);
        }
        if let Some(mentions) = $reply.allowed_mentions.clone() {
            $req = $req.allowed_mentions(mentions);
        }
    };
}

/// Response state of an interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResponseState {
    /// Nothing sent yet.
    Pending,
    /// Deferred (type 5 or 6); the real content comes as an edit.
    Deferred,
    /// Initial response sent.
    Responded,
}

/// Cloneable handle to answer one interaction. Clones share the state.
#[derive(Clone)]
pub struct InteractionHandle {
    inner: Arc<Inner>,
}

struct Inner {
    client: Client,
    id: Id<InteractionMarker>,
    application_id: Id<ApplicationMarker>,
    token: InteractionToken,
    kind: InteractionType,
    received: Instant,
    state: Mutex<ResponseState>,
}

impl std::fmt::Debug for InteractionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InteractionHandle")
            .field("id", &self.inner.id)
            .field("kind", &self.inner.kind)
            .finish_non_exhaustive()
    }
}

impl InteractionHandle {
    /// Handle for `interaction`; call right when it arrives (the 3-second
    /// window is measured from here).
    pub fn new(client: Client, interaction: &Interaction) -> Self {
        Self {
            inner: Arc::new(Inner {
                client,
                id: interaction.id,
                application_id: interaction.application_id,
                token: interaction.token.clone(),
                kind: interaction.kind,
                received: Instant::now(),
                state: Mutex::new(ResponseState::Pending),
            }),
        }
    }

    /// Current response state.
    pub async fn state(&self) -> ResponseState {
        *self.inner.state.lock().await
    }

    /// Time since the handle was created.
    pub fn elapsed(&self) -> Duration {
        self.inner.received.elapsed()
    }

    /// Send a message. After a defer this edits the deferred response.
    pub async fn respond(&self, reply: impl Into<Reply>) -> Result<(), InteractionError> {
        let reply = reply.into();
        let mut state = self.inner.state.lock().await;
        match *state {
            ResponseState::Pending => {
                check_reply(&reply)?;
                self.callback(
                    InteractionCallbackType::ChannelMessageWithSource,
                    Some(reply.into_callback_data()),
                )
                .await?;
                *state = ResponseState::Responded;
                Ok(())
            }
            ResponseState::Deferred => {
                self.edit_inner(reply).await?;
                *state = ResponseState::Responded;
                Ok(())
            }
            ResponseState::Responded => Err(InteractionError::AlreadyResponded),
        }
    }

    /// Show "thinking…" (type 5); answer later with [`Self::respond`] or
    /// [`Self::edit`]. A no-op if already deferred.
    pub async fn defer(&self) -> Result<(), InteractionError> {
        self.defer_with(
            InteractionCallbackType::DeferredChannelMessageWithSource,
            false,
        )
        .await
    }

    /// Like [`Self::defer`], but only the user sees the response.
    pub async fn defer_ephemeral(&self) -> Result<(), InteractionError> {
        self.defer_with(
            InteractionCallbackType::DeferredChannelMessageWithSource,
            true,
        )
        .await
    }

    /// Acknowledge a component click without a visible change (type 6);
    /// edit the message later with [`Self::edit`].
    pub async fn defer_update(&self) -> Result<(), InteractionError> {
        self.defer_with(InteractionCallbackType::DeferredUpdateMessage, false)
            .await
    }

    async fn defer_with(
        &self,
        kind: InteractionCallbackType,
        ephemeral: bool,
    ) -> Result<(), InteractionError> {
        let mut state = self.inner.state.lock().await;
        match *state {
            ResponseState::Pending => {
                let data = ephemeral.then(|| Reply::default().ephemeral().into_callback_data());
                self.callback(kind, data).await?;
                *state = ResponseState::Deferred;
                Ok(())
            }
            ResponseState::Deferred => Ok(()),
            ResponseState::Responded => Err(InteractionError::AlreadyResponded),
        }
    }

    /// Replace the message the component belongs to (type 7). After a
    /// defer this edits the response instead.
    pub async fn update_message(&self, reply: impl Into<Reply>) -> Result<(), InteractionError> {
        let reply = reply.into();
        let mut state = self.inner.state.lock().await;
        match *state {
            ResponseState::Pending => {
                check_reply(&reply)?;
                self.callback(
                    InteractionCallbackType::UpdateMessage,
                    Some(reply.into_callback_data()),
                )
                .await?;
                *state = ResponseState::Responded;
                Ok(())
            }
            ResponseState::Deferred => {
                self.edit_inner(reply).await?;
                *state = ResponseState::Responded;
                Ok(())
            }
            ResponseState::Responded => Err(InteractionError::AlreadyResponded),
        }
    }

    /// Autocomplete suggestions (type 8, max. 25).
    pub async fn autocomplete(
        &self,
        choices: impl IntoIterator<Item = Choice>,
    ) -> Result<(), InteractionError> {
        let choices: Vec<Choice> = choices.into_iter().collect();
        if choices.len() > crate::command::MAX_OPTIONS {
            return Err(InteractionError::InvalidResponse(
                format!("{} autocomplete choices; max. 25", choices.len()).into(),
            ));
        }
        let choices: Vec<CommandOptionChoice> = serde_json::to_value(&choices)
            .and_then(serde_json::from_value)
            .map_err(|e| InteractionError::InvalidResponse(e.to_string().into()))?;
        let mut data = InteractionCallbackData::default();
        data.choices = choices;
        self.initial(
            InteractionCallbackType::ApplicationCommandAutocompleteResult,
            Some(data),
        )
        .await
    }

    /// Open a modal (type 9). Title max. 45 characters.
    pub async fn modal(
        &self,
        custom_id: impl Into<Box<str>>,
        title: impl Into<Box<str>>,
        components: Vec<Component>,
    ) -> Result<(), InteractionError> {
        let title = title.into();
        if !(1..=45).contains(&title.chars().count()) {
            return Err(InteractionError::InvalidResponse(
                "modal title must be 1–45 characters".into(),
            ));
        }
        let mut data = InteractionCallbackData::default();
        data.custom_id = Some(custom_id.into());
        data.title = Some(title);
        data.components = components;
        self.initial(InteractionCallbackType::Modal, Some(data))
            .await
    }

    /// Open a [`Modal`] after checking its limits locally.
    pub async fn show_modal(&self, modal: Modal) -> Result<(), InteractionError> {
        modal.validate()?;
        self.modal(modal.custom_id, modal.title, modal.components)
            .await
    }

    async fn initial(
        &self,
        kind: InteractionCallbackType,
        data: Option<InteractionCallbackData>,
    ) -> Result<(), InteractionError> {
        let mut state = self.inner.state.lock().await;
        if *state != ResponseState::Pending {
            return Err(InteractionError::AlreadyResponded);
        }
        self.callback(kind, data).await?;
        *state = ResponseState::Responded;
        Ok(())
    }

    /// Edit the original response (after respond or defer).
    pub async fn edit(&self, reply: impl Into<Reply>) -> Result<Message, InteractionError> {
        let state = *self.inner.state.lock().await;
        if state == ResponseState::Pending {
            return Err(InteractionError::NotResponded);
        }
        let message = self.edit_inner(reply.into()).await?;
        let mut state = self.inner.state.lock().await;
        *state = ResponseState::Responded;
        Ok(message)
    }

    /// Delete the original response.
    pub async fn delete_original(&self) -> Result<(), InteractionError> {
        if *self.inner.state.lock().await == ResponseState::Pending {
            return Err(InteractionError::NotResponded);
        }
        self.check_token()?;
        self.inner
            .client
            .delete_interaction_response(self.inner.application_id, &self.inner.token)
            .await?;
        Ok(())
    }

    /// Send another message (after respond or defer).
    pub async fn followup(&self, reply: impl Into<Reply>) -> Result<Message, InteractionError> {
        if *self.inner.state.lock().await == ResponseState::Pending {
            return Err(InteractionError::NotResponded);
        }
        self.check_token()?;
        let reply = reply.into();
        let mut req = self
            .inner
            .client
            .create_followup_message(self.inner.application_id, &self.inner.token);
        apply_reply!(req, reply);
        if !reply.flags.is_empty() {
            req = req.flags(reply.flags);
        }
        Ok(req.send().await?)
    }

    /// Defer automatically if nothing was sent after `after` (use
    /// [`DEFAULT_AUTO_DEFER`]). [`Self::respond`] then edits the deferred
    /// response, so handlers do not need to care. Needs a Tokio runtime.
    pub fn auto_defer(&self, after: Duration, ephemeral: bool) {
        let handle = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(after).await;
            if handle.state().await == ResponseState::Pending {
                let result = if ephemeral {
                    handle.defer_ephemeral().await
                } else {
                    handle.defer().await
                };
                if let Err(e) = result {
                    tracing::debug!(error = %e, "auto-defer failed");
                }
            }
        });
    }

    async fn callback(
        &self,
        kind: InteractionCallbackType,
        data: Option<InteractionCallbackData>,
    ) -> Result<(), InteractionError> {
        if self.elapsed() > INITIAL_RESPONSE_WINDOW {
            return Err(InteractionError::Expired(
                "the 3-second window for the initial response has passed; defer earlier",
            ));
        }
        let response = InteractionResponse::new(kind, data);
        self.inner
            .client
            .create_interaction_response(self.inner.id, &self.inner.token, &response)
            .await?;
        Ok(())
    }

    async fn edit_inner(&self, reply: Reply) -> Result<Message, InteractionError> {
        self.check_token()?;
        let mut req = self
            .inner
            .client
            .update_interaction_response(self.inner.application_id, &self.inner.token);
        apply_reply!(req, reply);
        Ok(req.send().await?)
    }

    fn check_token(&self) -> Result<(), InteractionError> {
        if self.elapsed() > TOKEN_LIFETIME {
            return Err(InteractionError::Expired(
                "the interaction token is older than 15 minutes",
            ));
        }
        Ok(())
    }
}

fn check_reply(reply: &Reply) -> Result<(), InteractionError> {
    if let Some(content) = &reply.content {
        let n = content.chars().count();
        if n > CONTENT_LIMIT {
            return Err(InteractionError::InvalidResponse(
                format!("content is {n} characters; max. {CONTENT_LIMIT}").into(),
            ));
        }
    }
    if reply.embeds.len() > EMBED_COUNT_LIMIT {
        return Err(InteractionError::InvalidResponse(
            format!("{} embeds; max. {EMBED_COUNT_LIMIT}", reply.embeds.len()).into(),
        ));
    }
    let v2 = reply
        .components
        .as_deref()
        .is_some_and(cordex_model::component::needs_components_v2);
    if v2 && (reply.content.as_deref().is_some_and(|c| !c.is_empty()) || !reply.embeds.is_empty()) {
        return Err(InteractionError::InvalidResponse(
            "Components V2 messages cannot have content or embeds; put the text in a \
             TextDisplay (use Reply::layout)"
                .into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use cordex_http::mock::{self, MockResponse};
    use parking_lot::Mutex as SyncMutex;
    use serde_json::{Value, json};

    use super::*;

    #[test]
    fn components_v2_reply_rejects_content() {
        use cordex_model::component::{Container, TextDisplay};
        let layout = vec![Container::new([TextDisplay::new("hi")]).into()];
        assert!(check_reply(&Reply::layout(layout.clone())).is_ok());
        let err = check_reply(&Reply::new("text").components(layout)).unwrap_err();
        assert!(err.to_string().contains("TextDisplay"), "{err}");
    }

    const MESSAGE: &str = r#"{"id":"5","channel_id":"6","author":{"id":"2","username":"bot","discriminator":"0","global_name":null,"avatar":null},"content":"x","timestamp":"2024-01-01T00:00:00.000000+00:00","edited_timestamp":null,"tts":false,"mention_everyone":false,"mentions":[],"mention_roles":[],"attachments":[],"embeds":[],"pinned":false,"type":0}"#;

    type Log = Arc<SyncMutex<Vec<(String, String, Value)>>>;

    fn setup() -> (InteractionHandle, Log) {
        let log: Log = Arc::default();
        let seen = log.clone();
        let client = mock::client(move |req| {
            let body = serde_json::from_slice(req.body()).unwrap_or(Value::Null);
            let method = req.method().to_string();
            seen.lock()
                .push((method.clone(), req.uri().path().to_owned(), body));
            if method == "DELETE" || (method == "POST" && req.uri().path().contains("/callback")) {
                MockResponse::no_content()
            } else {
                MockResponse::json(MESSAGE)
            }
        })
        .unwrap();
        let interaction: Interaction = serde_json::from_value(json!({
            "id": "100", "application_id": "200", "type": 2, "token": "tok", "version": 1
        }))
        .unwrap();
        (InteractionHandle::new(client, &interaction), log)
    }

    #[tokio::test]
    async fn respond_once_then_followup() {
        let (h, log) = setup();
        h.respond(Reply::new("hi").ephemeral()).await.unwrap();
        assert_eq!(h.state().await, ResponseState::Responded);
        assert!(matches!(
            h.respond("again").await,
            Err(InteractionError::AlreadyResponded)
        ));
        h.followup("more").await.unwrap();
        let log = log.lock();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].0, "POST");
        assert_eq!(log[0].1, "/api/v10/interactions/100/tok/callback");
        assert_eq!(
            log[0].2,
            json!({"type": 4, "data": {"content": "hi", "flags": 64}})
        );
        assert_eq!(log[1].1, "/api/v10/webhooks/200/tok");
        assert_eq!(log[1].2, json!({"content": "more"}));
    }

    #[tokio::test]
    async fn respond_after_defer_edits_original() {
        let (h, log) = setup();
        h.defer_ephemeral().await.unwrap();
        h.defer().await.unwrap();
        h.respond("done").await.unwrap();
        let log = log.lock();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].2, json!({"type": 5, "data": {"flags": 64}}));
        assert_eq!(log[1].0, "PATCH");
        assert_eq!(log[1].1, "/api/v10/webhooks/200/tok/messages/@original");
        assert_eq!(log[1].2, json!({"content": "done"}));
    }

    #[tokio::test]
    async fn edit_and_followup_need_a_response() {
        let (h, log) = setup();
        assert!(matches!(
            h.edit("x").await,
            Err(InteractionError::NotResponded)
        ));
        assert!(matches!(
            h.followup("x").await,
            Err(InteractionError::NotResponded)
        ));
        assert!(matches!(
            h.delete_original().await,
            Err(InteractionError::NotResponded)
        ));
        assert!(log.lock().is_empty());
    }

    #[tokio::test]
    async fn too_long_content_is_rejected_locally() {
        let (h, log) = setup();
        let err = h.respond("a".repeat(2001)).await.unwrap_err();
        assert!(err.to_string().contains("2001 characters"));
        assert!(log.lock().is_empty());
        assert_eq!(h.state().await, ResponseState::Pending);
    }

    #[tokio::test]
    async fn autocomplete_and_modal() {
        let (h, log) = setup();
        h.autocomplete([Choice::string("Red", "red"), Choice::int("One", 1)])
            .await
            .unwrap();
        assert!(matches!(
            h.modal("m", "Title", vec![]).await,
            Err(InteractionError::AlreadyResponded)
        ));
        assert_eq!(
            log.lock()[0].2,
            json!({"type": 8, "data": {"choices": [
                {"name": "Red", "value": "red"}, {"name": "One", "value": 1}]}})
        );
    }

    #[tokio::test(start_paused = true)]
    async fn auto_defer_then_respond_edits() {
        let (h, log) = setup();
        h.auto_defer(DEFAULT_AUTO_DEFER, false);
        tokio::time::sleep(Duration::from_secs(3)).await;
        assert_eq!(h.state().await, ResponseState::Deferred);
        let log = log.lock();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].2, json!({"type": 5}));
    }

    #[tokio::test(start_paused = true)]
    async fn auto_defer_skipped_when_answered() {
        let (h, log) = setup();
        h.auto_defer(DEFAULT_AUTO_DEFER, false);
        h.respond("fast").await.unwrap();
        tokio::time::sleep(Duration::from_secs(3)).await;
        assert_eq!(log.lock().len(), 1);
    }
}
