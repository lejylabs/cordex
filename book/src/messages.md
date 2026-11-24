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

Build rich embeds with `EmbedBuilder` (validates Discord's character limits locally before sending):

```rust,no_run
use cordex::prelude::*;

let embed = EmbedBuilder::new()
    .title("Server Status")
    .description("All shards connected.")
    .color(0x57F287)
    .field("Latency", "24 ms", true)
    .build()?;

ctx.reply(Reply::new("").embed(embed)).await?;
```

## Pagination & Button Collectors

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

For custom interactive flows, `ctx.button_clicks(message_id, timeout)` streams clicks with `.idle_timeout(...)` and `.stop()`.

## Quick Shortcuts, Forwarding & Message Links

- **Send / DM:** `ctx.send(channel_id, "Hello!")` and `ctx.dm(user_id, "Hi!")`
- **React / Delete:** `msg_ctx.react("👍").await?` and `msg_ctx.delete().await?`
- **Forwarding:** `ctx.forward_message(source_channel_id, message_id, target_channel_id).await?`
- **Parse Discord message links:** `cordex::model::message::parse_message_link(url)` returns a typed `MessageLink { guild_id, channel_id, message_id }`.

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
