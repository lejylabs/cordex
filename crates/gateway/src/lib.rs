#![doc = include_str!("../README.md")]

pub mod connection;
pub mod error;
pub mod event_flags;
pub mod identify;
pub mod manager;
pub mod mock;
pub mod shard;
pub mod spawn;

mod intents_check;
mod outbound;
mod queue;
mod session;
mod standby;
mod types;
mod zlib;

pub use connection::{
    Connector, DEFAULT_MAX_FRAME_BYTES, Incoming, Outgoing, WsConnection, WsConnector,
};
pub use error::GatewayError;
pub use event_flags::EventTypeFlags;
pub use identify::{
    IDENTIFY_INTERVAL, IDENTIFY_MARGIN, IDENTIFY_SPACING, IdentifyQueue, ImmediateIdentifyQueue,
    LocalIdentifyQueue,
};
pub use manager::{ManagerEvents, ShardManager, ShardManagerBuilder};
pub use shard::{GatewayBotSource, Shard, ShardBuilder};
pub use spawn::{CurrentRuntime, RuntimeHandles, ShardTask, Spawner};
pub use standby::{STANDBY_STREAM_CAPACITY, Standby};
pub use types::{
    Backpressure, CloseMode, DEFAULT_GATEWAY_URL, DEFAULT_MAX_MESSAGE_BYTES, HEARTBEAT_RESERVE,
    LIBRARY_NAME, Latency, MAX_PAYLOAD_BYTES, ManagerEvent, OUTBOUND_LIMIT, OUTBOUND_WINDOW,
    ResumeInfo, ShardCounters, ShardId, ShardState, ShardStatus, USER_OUTBOUND_LIMIT,
};

#[cfg(test)]
mod tests;
