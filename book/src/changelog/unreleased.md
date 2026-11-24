# Unreleased

Changes currently on `master` and scheduled for the next release.

- **`cordex-model`:**
  - Added `EmbedAudio` and `Embed.audio` (pinned `discord-api-docs@5a61ef8ebaf5`, #3).
  - Added `parse_invite_url(url)` (re-exported at crate root) and `ParseError::InviteUrl` for parsing `discord.gg/{code}` and `discord.com/invite/{code}` URLs.
