//! Provider-oriented YouTube discovery primitives.
//!
//! The official YouTube Data API provider exposes video search, metadata, and
//! popular-video charts. It does not expose the full YouTube Music catalog,
//! Music-specific suggestions, or personalized Music recommendations. Playback
//! is handed off to YouTube's player; this crate does not resolve audio streams.

pub mod error;
pub mod models;
pub mod provider;
pub mod providers;
pub mod recommendations;

pub use error::{Error, Result};
pub use models::*;
pub use provider::*;
