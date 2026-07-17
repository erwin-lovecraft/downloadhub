//! YouTube Data API v3 client: keyword search, playlist listing, and video
//! metadata.
//!
//! Calls the REST API directly with `reqwest` + `serde` rather than the
//! generated `google-youtube3` crate. `search.list`/`videos.list`/
//! `playlistItems.list` are all plain API-key-authenticated GET requests
//! with a small response shape; the generated client would additionally
//! pull in `yup-oauth2` and an authenticator/hyper-connector setup that's
//! pure friction for calls that don't need user auth at all. See
//! `docs/ARCHITECTURE.md`.
//!
//! Layout, one responsibility per file:
//!
//! - [`client`]: HTTP orchestration (endpoints, pagination, batching)
//! - [`models`]: the public [`VideoSummary`] model
//! - [`response`]: raw API wire shapes + conversion into the model
//! - [`duration`]: ISO-8601 duration parsing

mod client;
mod duration;
mod models;
mod response;

pub use client::{YoutubeClient, YoutubeError};
pub use models::VideoSummary;
