# Messages, Embeds & Helpers

`cordex` includes ergonomic builders and helpers for formatting messages, embeds, pagination, message history, forwarding, and typing indicators.

## `MessageBuilder` (Safe Markdown Formatting)

`MessageBuilder` (in `cordex::prelude`) lets you compose formatted Discord messages while safely escaping untrusted user input:

```rust,no_run
use cordex::prelude::*;

let text = MessageBuilder::new()
    .push_bold("Welcome ")
    .push_safe(username) // escapes *, _, ~, `, |, \, >
    .push_line("!")
    .push_quote_line("Read the rules in #info.")
    .push_codeblock("rust", "println!(\"hello\");")
    .build();
```

Available helpers include `push`, `push_line`, `push_safe`, `push_bold[_safe]`, `push_italic[_safe]`, `push_underline[_safe]`, `push_strike[_safe]`, `push_spoiler[_safe]`, `push_mono[_safe]`, `push_quote_line`, `push_codeblock`, `push_named_link[_safe]`, `mention`, and `push_timestamp`.

## Embeds & Attachments

Build rich embeds with `Embed` (validates Discord's character limits locally before sending), using either `Embed::new()` + `colors::*` or preset constructors (`Embed::success`, `Embed::error`, `Embed::info`, `Embed::warn`):

```rust,no_run
use cordex::prelude::*;

let embed = Embed::success("All shards connected.")?
    .title("Server Status")?
    .field("Latency", "24 ms", true)?;

ctx.reply(Reply::embed(embed)).await?;
```

## Pagination, Confirmations & Interactive Flows

For multi-page responses, `CommandContext::paginate` renders `◀` / `1 / N` / `▶` buttons, restricts clicks to the command author, resets the idle timeout on every click, and disables the buttons when the timeout expires:

```rust,no_run
use std::time::Duration;
use cordex::prelude::*;

async fn pages(ctx: CommandContext) -> cordex::Result<()> {
    let pages = [
        Reply::new("Page 1: Overview"),
        Reply::new("Page 2: Commands"),
        Reply::new("Page 3: Settings"),
    ];
    ctx.paginate(pages, Duration::from_secs(60)).await
}
```

Additional built-in inline flows on `CommandContext`:

- `ctx.confirm("Delete this item?", Duration::from_secs(30)).await? -> bool` (or `confirm_with` for custom button labels)
- `ctx.select("Pick one", options, Duration::from_secs(30)).await? -> Option<Vec<String>>`
- `ctx.prompt("Reply in chat…", Duration::from_secs(60)).await? -> Option<Message>`
- `ctx.ask_modal(modal, Duration::from_secs(60)).await? -> Option<ModalContext>` (also on `ComponentContext`)

For custom interactive flows, `ctx.button_clicks(message_id, timeout)` streams clicks with `.idle_timeout(...)` and `.stop()`.

## Quick Shortcuts, Forwarding & Message Links

- **Send / DM:** `ctx.send(channel_id, "Hello!")` and `ctx.dm(user_id, "Hi!")`
- **React / Delete:** `msg_ctx.react("👍").await?` and `msg_ctx.delete().await?`
- **Forwarding:** `ctx.forward_message(source_channel_id, message_id, target_channel_id).await?`
- **Parse Discord links:** `cordex::model::message::parse_message_link(url)` returns a typed `MessageLink { guild_id, channel_id, message_id }`; `cordex::model::parse_invite_url(url)` extracts invite codes (`discord.gg/…`, `discord.com/invite/…`); `cordex::model::parse_webhook_url(url)` parses webhook execute URLs.

## Typing Indicator (RAII Guard)

`ctx.start_typing(channel_id)` spawns a background task that refreshes Discord's typing indicator every few seconds and stops automatically when the guard is dropped:

```rust,no_run
use cordex::prelude::*;

async fn work(ctx: CommandContext, channel_id: Id<ChannelMarker>) -> cordex::Result<()> {
    let _typing = ctx.start_typing(channel_id);
    // ... long-running task ...
    Ok(()) // typing stops automatically here
}
```

## Message History Paginator

Iterate backwards through a channel's message history without manual `before` pagination:

```rust,no_run
use cordex::prelude::*;

async fn scan(client: &Client, channel_id: Id<ChannelMarker>) -> cordex::Result<()> {
    let mut history = client.messages_iter(channel_id);
    while let Some(msg) = history.next().await.transpose()? {
        println!("{}: {}", msg.author.username, msg.content);
    }
    Ok(())
}
```
