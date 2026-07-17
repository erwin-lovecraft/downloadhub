//! Pending agent actions: queue-mutating requests made by an external AI
//! agent (via the `mcp-server` binary) that must be explicitly approved by
//! the user in the running desktop app before they execute.
//!
//! The MCP server never mutates the queue or starts a download itself — it
//! only inserts rows here (status `pending`). The desktop app polls this
//! table, shows each pending action to the user, and on approval executes
//! the underlying operation itself, recording the outcome back on the row.
//! Rows live in the same SQLite database as the queue (`QueueStore`), which
//! both processes already share — see `docs/ARCHITECTURE.md`.
//!
//! Layout mirrors `core::queue`: [`action`] holds the domain types,
//! [`repository`] the SQL, and [`store`] the async facade (as extra
//! methods on `QueueStore`).

mod action;
mod repository;
mod store;

pub use action::{AgentActionError, AgentActionRequest, AgentActionStatus, PendingAgentAction};
