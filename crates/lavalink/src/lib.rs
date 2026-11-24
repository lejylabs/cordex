#![doc = include_str!("../README.md")]

pub mod client;
pub mod error;
pub mod model;

pub use client::LavalinkClient;
pub use error::LavalinkError;
pub use model::{
    LavalinkEvent, LoadError, LoadResult, NodeConfig, NodeStats, Player, PlayerEvent, PlayerState,
    PlaylistData, PlaylistInfo, Track, TrackInfo, UpdatePlayer, UpdatePlayerTrack,
};
