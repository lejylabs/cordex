//! Composable `around`-middleware for application commands.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::Result;
use crate::bot::{CommandContext, Handler};

/// Continuation passed to an `around_command` middleware.
///
/// Call [`Next::run`] to invoke the next middleware or the command handler itself.
pub struct Next {
    handler: Handler<CommandContext>,
}

impl Next {
    pub(crate) fn new(handler: Handler<CommandContext>) -> Self {
        Self { handler }
    }

    /// Run the remaining middleware chain and command handler.
    pub async fn run(self, ctx: CommandContext) -> Result<()> {
        (self.handler)(ctx).await
    }
}

/// Boxed `around_command` middleware function.
pub(crate) type AroundCommand = Arc<
    dyn Fn(CommandContext, Next) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync,
>;

/// Wrap `handler` in a slice of `around` middlewares (first registered runs outermost).
pub(crate) fn wrap_around(
    mut handler: Handler<CommandContext>,
    middlewares: &[AroundCommand],
) -> Handler<CommandContext> {
    for mw in middlewares.iter().rev() {
        let mw = Arc::clone(mw);
        let next_handler = handler;
        handler = Arc::new(move |ctx: CommandContext| {
            let mw = Arc::clone(&mw);
            let next = Next::new(Arc::clone(&next_handler));
            Box::pin(async move { mw(ctx, next).await })
        });
    }
    handler
}
