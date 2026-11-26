//! Offline testing harness and gateway-replay runner for `cordex` bots.
//!
//! Drive any [`cordex::Bot`] completely offline in `cargo test` with an
//! in-memory mock HTTP recorder, ergonomic interaction/event builders,
//! deterministic `tokio::time::pause()` support, and `.jsonl` gateway replay.

pub mod error;
pub mod harness;
pub mod recorder;
pub mod result;

pub use cordex_gateway::recorder::{Anonymizer, RecordedFrame, TrafficRecorder};
pub use cordex_http::mock::MockResponse;
pub use error::TestError;
pub use harness::{
    AutocompleteBuilder, ComponentBuilder, MessageBuilder, ModalBuilder, SlashBuilder, TestBot,
    settle,
};
pub use recorder::{MockHttp, RecordedRequest};
pub use result::{EventResult, InteractionResult};
