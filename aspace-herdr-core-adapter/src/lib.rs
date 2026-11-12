//! A'Space adapter for herdr-core — D-N053 pas F1 (design signé Clara, D-F1-1/D-F1-2).
//!
//! The adapter is the **client** side of herdr-core only:
//!
//! * the framed protocol (`wire`: `[channel u32 LE][kind u8][len u32 LE][payload]`,
//!   channel 0 carries JSON-RPC) and the 32-method OpenRPC API (`api`),
//! * the socket client (`client`: `connect_existing` / `require_existing` /
//!   `call` — never `connect_or_start`, which would spawn a core),
//! * the client CLI (`hc`, `src/bin/hc.rs`): every `hc` verb except
//!   `server start` and `update`.
//!
//! Explicitly **not** re-exported, so no A'Space code can reach them through
//! this crate:
//!
//! * `server` — `server::serve()` is the `hc` daemon (lock `core.lock`,
//!   listen, `Core::start`, `recover_kept`, `run_jobs`, launchd/systemd
//!   registration). Retired by F8 pas 8; it never comes back (D-F1-2).
//! * `cli` — upstream `cli::main()` dispatches `server start`, `update`,
//!   `serve` and `hold`, and auto-starts a daemon (`platform::daemon_startup`).
//!   The adapter binary is its own CLI and refuses those verbs.
//!
//! Measured at the gel `cd12ddd` (v0.1.18 + 10): `lib.rs` declares **35**
//! `pub mod` (the design's "30" is stale — re-measured here). This crate
//! re-exports **33** of them: all except `server` and `cli`.
//!
//! Forced config (D-F1-5, H-F1-4): `notes = false`. Upstream default is ON
//! (`config.rs`: "Notes to parent agents […] on or off (default on)");
//! the bundled `config.toml` next to this crate forces it OFF, and
//! `forced_notes()` states it in code.
//!
//! Real default socket (Discover E-F1-4, H-F1-7):
//! `$XDG_RUNTIME_DIR/herdr-core/default.sock` (session `default`,
//! `HC_SESSION` to change it). `HC_SOCKET` wins over the config file, which
//! wins over the default. `/tmp/herdr-core.sock` was the retired V5
//! supervisor's override, not the `hc` default.

pub use herdr_core::addr;
pub use herdr_core::api;
pub use herdr_core::attach;
pub use herdr_core::client;
pub use herdr_core::config;
pub use herdr_core::events;
pub use herdr_core::exec;
pub use herdr_core::files;
pub use herdr_core::foreground;
pub use herdr_core::holder;
pub use herdr_core::holder_proto;
pub use herdr_core::input;
pub use herdr_core::keys;
pub use herdr_core::locale;
pub use herdr_core::machines;
pub use herdr_core::notes;
pub use herdr_core::paths;
pub use herdr_core::picker;
pub use herdr_core::platform;
pub use herdr_core::put;
pub use herdr_core::relay;
pub use herdr_core::releases;
pub use herdr_core::remote;
pub use herdr_core::rpc;
pub use herdr_core::runtime;
pub use herdr_core::status;
pub use herdr_core::stitch;
pub use herdr_core::sweep;
pub use herdr_core::templates;
pub use herdr_core::terminals;
pub use herdr_core::viewers;
pub use herdr_core::wait;
pub use herdr_core::wire;

/// Frozen upstream commit (the gel, D-N053 §1): `cd12ddd` (v0.1.18 + 10).
pub const PINNED_UPSTREAM_COMMIT: &str = "cd12ddd88bc11490228343291dc1096c9ed3bf77";
/// Cordis Fiber bind (8L) on `aspace-v5-native`, re-applied verbatim above
/// the adapter (F8 D4). The pin is the adapter commit, not the bind (§0).
pub const BIND_COMMIT: &str = "14a37722ec6ba968b0bfdaacce4ec76b21b0b4ca";
/// Fiber address of this module.
pub const FIBER_ADDRESS: &str = "fiber:a0:herdr-core";
/// Adapter version.
pub const ADAPTER_VERSION: &str = "0.1.0";

/// `notes` as forced by the adapter (D-F1-5): OFF. Upstream default is ON;
/// with this OFF, `hc` types nothing on its own (Discover §6 T7).
pub fn forced_notes() -> bool {
    false
}

/// The real default socket for a session, before overrides
/// (`HC_SOCKET` > config `socket` > this default). Mirrors `paths.rs`:
/// `$XDG_RUNTIME_DIR/herdr-core/{session}.sock`.
pub fn default_socket_path(
    runtime_dir: &std::path::Path,
    session: &str,
) -> std::path::PathBuf {
    runtime_dir.join("herdr-core").join(format!("{session}.sock"))
}
