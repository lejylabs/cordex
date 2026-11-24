# HTTP Interactions

Instead of (or alongside) a gateway connection, Discord can POST interactions to an HTTPS endpoint.

## Standalone Hyper server

Enable `interactions-server` on `cordex`:

```toml
cordex = { version = "0.1.7", default-features = false, features = ["interactions-server"] }
```

`InteractionEndpoint` enforces:

- Mandatory Ed25519 signature verification (`X-Signature-Ed25519` + `X-Signature-Timestamp`) on the raw request body before JSON parsing.
- Automatic `PING` → `PONG` responses.
- 1 MiB body limit (`413`), 3-second handler deadline (`503`), and `405` for non-POST requests.

See `crates/facade/examples/interactions_http.rs`.

## Inside another web framework

Enable `interactions-verify` and call `cordex::interactions::verify::verify_signature(&public_key, timestamp, signature, raw_body)` inside Axum, Actix, or any other HTTP server.
