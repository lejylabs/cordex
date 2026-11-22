//! Gateway protocol types (events/gateway.mdx, events/gateway-events.mdx,
//! topics/opcodes-and-status-codes.mdx).
//!
//! Receive side: [`Envelope`] is parsed first with `d` left as raw JSON, then
//! only the events that are consumed are parsed fully (PLAN.md §5.4).
//! Send side: [`command`] holds the serialize-only payloads.

pub mod bot;
pub mod close_code;
pub mod command;
pub mod event;
pub mod intents;
pub mod opcode;
pub mod payload;
pub mod presence;

pub use bot::{GatewayBot, SessionStartLimit};
pub use close_code::{CloseAction, CloseCode};
pub use event::Event;
pub use intents::Intents;
pub use opcode::OpCode;
pub use payload::{Envelope, Hello};
