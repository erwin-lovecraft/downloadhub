//! YouTube Data API v3 client: keyword search, playlist listing, and video
//! metadata. Calls the REST API directly with `reqwest` + `serde` rather than
//! the generated `google-youtube3` crate — see `docs/ARCHITECTURE.md`.

mod client;
mod duration;
mod models;
mod response;

pub use client::{YoutubeClient, YoutubeError};
pub use models::VideoSummary;
