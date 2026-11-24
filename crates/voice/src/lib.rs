#![doc = include_str!("../README.md")]

pub mod error;
pub mod gateway;
pub mod manager;
pub mod types;

pub use error::VoiceError;
pub use gateway::VoiceGateway;
pub use manager::{DEFAULT_EVENT_CAPACITY, VoiceManager};
pub use types::{LavalinkVoiceState, VoiceConnectionInfo, VoiceEvent, VoiceOptions};
