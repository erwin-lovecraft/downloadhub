# core/ — `downloadhub-core`

All of downloadhub's business logic. **Has no `tauri` dependency and must not
gain one**, so both `src-tauri` and `mcp-server` can reuse it unchanged. Anything
Tauri-specific (commands, events, managed state) belongs in `src-tauri`.

Rationale for the decisions below lives in
[`../docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md).

## Modules

| Module | Responsibility |
| --- | --- |
| `youtube` | YouTube Data API v3 client: `search.list`, `videos.list`, `playlistItems.list`. Direct `reqwest` + `serde`, not the generated crate. |
| `stream` | `y7dl` wrapper (`StreamClient`), format DTOs, and `FormatPreference` selection. |
| `queue` | SQLite-backed download queue: entry types, schema, SQL, async store. |
| `enqueue` | Bulk add / re-format driven by a `FormatPreference` rather than an itag. |
| `download` | Download orchestration, progress reporting, output naming, the `Transcode` seam. |
| `settings` | `AppSettings` JSON blob load/save. |
| `paths` | App data dir, queue DB path, settings path, OS Downloads dir. |
| `secrets` | Credential resolution: runtime env, then compile-time `option_env!`. |
| `auth` | Google OAuth loopback flow + keychain token storage. **Currently dormant** — nothing calls it. |

## Layout convention

Each multi-file module is one responsibility per file, with `mod.rs` reduced to
docs plus re-exports so public paths stay flat
(`downloadhub_core::queue::QueueStore`, not `...::queue::store::QueueStore`).
Keep it that way when adding files.

- `queue/` → `entry` (types), `schema` (DDL/migrations), `repository` (SQL),
  `store` (connection + async facade)
- `download/` → `runner` (orchestration), `progress`, `output` (filenames),
  `transcode` (the trait)
- `auth/` → `flow` (OAuth wire), `tokens` (models/expiry), `keychain`
- `youtube/` → `client` (HTTP), `models` (public), `response` (wire shapes),
  `duration` (ISO 8601)
- `stream/` → `client`, `models`

## Rules specific to this crate

- **No Tauri.** Progress and other callbacks are plain `FnMut`, so the caller
  decides whether they become Tauri events, channel sends, or test spies.
- **No concrete transcoder.** `download::Transcode` is an object-safe trait;
  `core` decides *when* post-processing runs, never *how*. Errors cross the seam
  as a boxed `BoxError` that `core` only displays.
- **Platform-path-agnostic APIs.** `QueueStore::open` and `settings::load` take
  a `&Path`. `paths` is where platform resolution lives, and it exists in `core`
  only because both binaries must agree on the same directory.
- **SQL only in repository files.** `QueueRepository` is synchronous and borrows
  an already-locked connection; `QueueStore` owns the connection and does the
  locking, `spawn_blocking`, and error wrapping. Never mix the two in one file.
- **`rusqlite` is blocking** — every store method must go through
  `spawn_blocking`.
- **No `unwrap()`/`panic!` on I/O or network paths.** Return `Result`.
- **GPL:** this crate links `y7dl` (GPL-3.0-or-later). Do not add proprietary or
  closed dependencies here.

## Testing

This is where the tests belong. Prioritize queue state transitions —
`QueueStore::open_in_memory` exists for exactly that (it skips WAL, which
doesn't apply to in-memory databases).
