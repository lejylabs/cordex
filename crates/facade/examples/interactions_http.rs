//! HTTP interactions instead of the gateway: Discord POSTs interactions to
//! your HTTPS URL (set it as "Interactions Endpoint URL" in the developer
//! portal). Signatures are checked with your application's public key;
//! PING is answered on its own.
//!
//! ```text
//! PUBLIC_KEY=... cargo run -p cordex --example interactions_http --features interactions-server
//! ```

use cordex::interactions::server::InteractionEndpoint;
use cordex::interactions::verify::PublicKey;
use cordex::model::interaction::{InteractionData, InteractionResponse};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = PublicKey::from_hex(&std::env::var("PUBLIC_KEY")?)?;
    let endpoint = InteractionEndpoint::new(key, |interaction| async move {
        let name = match &interaction.data {
            Some(InteractionData::Command(data)) => data.name.to_string(),
            _ => String::new(),
        };
        InteractionResponse::message(format!("Hello from HTTP, you used /{name}"))
    });
    // Put a TLS-terminating proxy (nginx, Caddy, a tunnel) in front.
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    println!("listening on :8080");
    endpoint.serve(listener).await;
    Ok(())
}
