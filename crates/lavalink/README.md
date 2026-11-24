# `cordex-lavalink`

Pure-Rust **Lavalink v4** client for `cordex`, built on top of `cordex-voice`.

- Connects to Lavalink v4's `/v4/websocket` endpoint and tracks the active `sessionId`.
- Provides typed REST helpers: `load_tracks`, `update_player`, `play`, `stop`, `set_paused`, `set_volume`, `destroy_player`, and `update_voice`.
- Bridges `cordex-voice`'s `VoiceManager` automatically via `LavalinkClient::attach_voice`: whenever Discord sends `Connected`, `ServerUpdated`, or `Disconnected`, Lavalink is updated automatically.
- 100% Pure Rust — no C/FFI, Opus, or `FFmpeg` dependencies.
