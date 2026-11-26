//! Integration tests for `cordex-test` (F.1 offline harness & F.2 gateway replay).

use std::num::NonZeroU64;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use cordex::prelude::*;
use cordex_interactions::Choice;
use cordex_model::id::{ChannelId, MessageId, UserId};
use cordex_test::{Anonymizer, TestBot, TrafficRecorder, settle};

#[tokio::test]
async fn slash_button_modal_autocomplete_and_message() -> Result<(), Box<dyn std::error::Error>> {
    let bot = Bot::new("mock")
        .command(
            Command::slash("greet", "Greet a user"),
            |ctx: CommandContext| async move {
                let target = ctx.user_opt("user")?.map_or(0, |u| u.id.get());
                let note = ctx.string_opt("note")?.unwrap_or("hi");
                ctx.reply_ephemeral(format!("Hello <@{target}>: {note}"))
                    .await
            },
        )
        .on_button_prefix("ticket:close:", |ctx: ComponentContext| async move {
            let tail = ctx.matched_tail();
            ctx.reply(format!("Closed ticket {tail}")).await
        })
        .on_modal("feedback", |ctx: ModalContext| async move {
            let text = ctx.value("text").unwrap_or("");
            ctx.reply(format!("Thanks: {text}")).await
        })
        .autocomplete("search", |ctx: AutocompleteContext| async move {
            let q = ctx.focused_value();
            let choices = ["cordex", "core"]
                .into_iter()
                .filter(|c| c.starts_with(q.as_str()))
                .map(|c| Choice::string(c, c))
                .collect::<Vec<_>>();
            ctx.suggest(choices).await
        })
        .on_message(|ctx: MessageContext| async move {
            if ctx.content() == "!ping" {
                ctx.reply("Pong!").await?;
            }
            Ok(())
        });

    let harness = TestBot::new(bot).await?;

    let alice =
        UserId::new(400_000_000_000_000_042).unwrap_or_else(|| UserId::from(NonZeroU64::MIN));
    let res = harness
        .slash("greet")
        .user_opt("user", alice)
        .string_opt("note", "welcome")
        .send()
        .await;
    res.assert_replied("Hello <@400000000000000042>: welcome");
    res.assert_ephemeral();

    let msg_id =
        MessageId::new(600_000_000_000_000_001).unwrap_or_else(|| MessageId::from(NonZeroU64::MIN));
    let btn = harness.click_button(msg_id, "ticket:close:42").send().await;
    btn.assert_replied("Closed ticket 42");

    let modal = harness
        .submit_modal("feedback", [("text", "Super!")])
        .send()
        .await;
    modal.assert_replied("Thanks: Super!");

    let ac = harness
        .trigger_autocomplete("search", "query", "cor")
        .send()
        .await;
    ac.assert_autocomplete_choices(&["cordex", "core"]);

    let chan =
        ChannelId::new(300_000_000_000_000_099).unwrap_or_else(|| ChannelId::from(NonZeroU64::MIN));
    let msg = harness.send_message(chan, "!ping").send().await;
    msg.assert_sent_message("Pong!");

    Ok(())
}

#[tokio::test(start_paused = true)]
async fn paused_time_triggers_auto_defer() -> Result<(), Box<dyn std::error::Error>> {
    let bot = Bot::new("mock").command(
        Command::slash("slow", "Slow command"),
        |ctx: CommandContext| async move {
            tokio::time::sleep(Duration::from_secs(4)).await;
            ctx.reply("Finished after defer").await
        },
    );

    let harness = TestBot::new(bot).await?;
    let start = harness.mock_http().len();

    // Start the interaction, then advance virtual time past the 2.5s auto-defer
    // and the 4s handler sleep.
    let fut = harness.slash("slow").send();
    tokio::pin!(fut);
    let _ = tokio::time::timeout(Duration::from_millis(10), &mut fut).await;
    tokio::time::advance(Duration::from_secs(5)).await;
    settle().await;

    let result =
        cordex_test::InteractionResult::from_requests(harness.mock_http().requests_since(start));
    result.assert_deferred();
    result.assert_replied("Finished after defer");
    Ok(())
}

#[tokio::test]
async fn record_anonymize_and_replay_jsonl() -> Result<(), Box<dyn std::error::Error>> {
    let seen = Arc::new(AtomicUsize::new(0));
    let seen_clone = Arc::clone(&seen);
    let bot = Bot::new("mock").on_message(move |_ctx: MessageContext| {
        let seen = Arc::clone(&seen_clone);
        async move {
            seen.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    });

    let recorder = TrafficRecorder::in_memory().with_anonymizer(Anonymizer::new());
    recorder.record_dispatch(
        0,
        "MESSAGE_CREATE",
        &serde_json::json!({
            "id": "911111111111111111",
            "channel_id": "922222222222222222",
            "author": {
                "id": "933333333333333333",
                "username": "real_user",
                "discriminator": "0",
                "bot": false
            },
            "content": "sensitive production message",
            "timestamp": "2026-01-01T00:00:00.000000+00:00",
            "tts": false,
            "mention_everyone": false,
            "mentions": [],
            "mention_roles": [],
            "attachments": [],
            "embeds": [],
            "pinned": false,
            "type": 0
        }),
    )?;

    let jsonl = recorder.memory_lines().join("\n");
    assert!(!jsonl.contains("real_user"));
    assert!(!jsonl.contains("sensitive production message"));

    let harness = TestBot::new(bot).await?;
    let replayed = harness.replay_jsonl_str(&jsonl).await?;
    assert_eq!(replayed, 1);
    assert_eq!(seen.load(Ordering::Relaxed), 1);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn scheduler_runs_periodically_and_shuts_down() -> Result<(), Box<dyn std::error::Error>> {
    let ticks = Arc::new(AtomicUsize::new(0));
    let ticks_clone = Arc::clone(&ticks);
    let bot = Bot::new("mock").every_immediate(Duration::from_secs(10), move |_ctx: Context| {
        let ticks = Arc::clone(&ticks_clone);
        async move {
            ticks.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    });

    let harness = TestBot::new(bot).await?;
    let handle = harness.start_schedulers();
    settle().await;
    assert_eq!(ticks.load(Ordering::Relaxed), 1);

    tokio::time::advance(Duration::from_secs(10)).await;
    settle().await;
    assert_eq!(ticks.load(Ordering::Relaxed), 2);

    tokio::time::advance(Duration::from_secs(10)).await;
    settle().await;
    assert_eq!(ticks.load(Ordering::Relaxed), 3);

    handle.shutdown(Duration::from_secs(1)).await;
    Ok(())
}
