//! Facade crate: re-exports the `drs-*` crates behind features
//! (PLAN.md §4.3).

pub use drs_model as model;

#[cfg(feature = "cache")]
pub use drs_cache as cache;
#[cfg(feature = "gateway")]
pub use drs_gateway as gateway;
#[cfg(feature = "http")]
pub use drs_http as http;
#[cfg(feature = "interactions")]
pub use drs_interactions as interactions;
