# Components & Modals

`cordex` provides typed builders with local validation for every Discord component, including **Components V2** and modal inputs.

## Classic components

```rust,no_run
use cordex::prelude::*;

let row = ActionRow::new([
    Button::primary("ok", "OK"),
    Button::danger("cancel", "Cancel"),
]);
```

## Components V2

`cordex` sets the `IS_COMPONENTS_V2` message flag automatically whenever a message, interaction response, follow-up, or webhook execution contains V2 components (`Container`, `Section`, `TextDisplay`, `Thumbnail`, `MediaGallery`, `FileComponent`, `Separator`). Combining V2 components with plain `content` or `embeds` is rejected locally before sending.

```rust,no_run
use cordex::model::component::{Container, Section, Separator, TextDisplay, Thumbnail};
use cordex::prelude::*;

let layout = Container::new([
    TextDisplay::new("## Status").into(),
    Separator::new().into(),
    Section::new(
        [TextDisplay::new("All systems operational.").into()],
        Thumbnail::new("https://example.com/icon.png"),
    )
    .into(),
]);

let reply = Reply::layout([layout.into()]);
```

See `crates/facade/examples/components_v2.rs` for a full runnable bot.

## Modals

Build modals with `Modal::new(custom_id, title)` and read submitted values in `ModalContext` (`ctx.value(id)` for text inputs and radio groups, `ctx.checked(id)` for checkboxes). You can also wait for a modal inline with `ctx.wait_for_modal(custom_id, timeout)`. See `crates/facade/examples/embeds_and_modals.rs`.

## Prefix Routing, Fallbacks & V2 Limits

- `Bot::on_component` and `Bot::on_modal` match by exact `custom_id` first, then by the longest `:`-delimited prefix (e.g. `"giveaway:join:42"` checks `"giveaway:join:42"`, `"giveaway:join"`, then `"giveaway"`).
- Register `Bot::on_unknown_component` / `Bot::on_unknown_modal` to handle expired or unrecognized buttons/modals gracefully.
- Validate Components V2 message limits locally before sending with `Component::validate_v2_limits(&components)` (`<= 10` top-level, `<= 40` total nested components).
