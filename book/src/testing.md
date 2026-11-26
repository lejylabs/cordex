# Offline Testing & Gateway Replay (`cordex-test`)

`cordex-test` lets you test your `Bot`, slash commands, buttons, modals, autocomplete, schedulers, and real production event sequences completely offline in `cargo test` — with zero network I/O.

## Setup

Add `cordex-test` under `[dev-dependencies]`:

```toml
[dev-dependencies]
cordex-test = "0.2.1"
```

## Testing Commands, Components & Modals

`TestBot::new(bot)` wraps your normal `Bot` without changing any production code:

```rust
use cordex::prelude::*;
use cordex_test::TestBot;

#[tokio::test]
async fn greet_command_replies_ephemerally() -> Result<(), Box<dyn std::error::Error>> {
    let bot = Bot::new("mock").command(
        Command::slash("greet", "Greet a user"),
        |ctx: CommandContext| async move {
            let note = ctx.string_opt("note")?.unwrap_or("hi");
            ctx.reply_ephemeral(format!("Hello: {note}")).await
        },
    );

    let harness = TestBot::new(bot).await?;
    let res = harness.slash("greet").string_opt("note", "welcome").send().await;
    res.assert_replied("Hello: welcome");
    res.assert_ephemeral();
    Ok(())
}
```

Available builders and assertions:
- `harness.slash(...)`, `click_button(...)`, `submit_modal(...)`, `trigger_autocomplete(...)`, `send_message(...)`
- `seed_guild(...)`, `seed_channel(...)`, `seed_user(...)`, `seed_member(...)`
- `assert_replied`, `assert_ephemeral`, `assert_deferred`, `assert_modal`, `assert_autocomplete_choices`, `assert_sent_message`
- `harness.mock_http().inject_error(StatusCode::FORBIDDEN, 50013, "Missing Permissions")`

## Deterministic Time (`tokio::time::pause`)

Auto-defer (2.5 s), cooldowns, `confirm`/`paginate` timeouts, and `Bot::every` schedulers run deterministically in milliseconds under `#[tokio::test(start_paused = true)]`:

```rust
let handle = harness.start_schedulers();
tokio::time::advance(Duration::from_secs(10)).await;
cordex_test::settle().await;
handle.shutdown(Duration::from_secs(1)).await;
```

## Recording & Replaying Real Gateway Traffic

Enable the opt-in `record` feature on `cordex-gateway` to capture dispatch frames as `.jsonl` with consistent 1:1 anonymization (`TrafficRecorder` + `Anonymizer`), then replay them in tests:

```rust
let count = harness.replay_jsonl("tests/fixtures/thread_edge_case.jsonl").await?;
```
