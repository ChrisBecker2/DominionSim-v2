//! Human-readable scripted strategies, a massively parallel batch runner, and a CLI, built on
//! top of `dominion-engine`.
//!
//! - [`expr`] — the small allocation-free expression language used by buy conditions.
//! - [`strategy`] — the TOML strategy format, compiled into a [`strategy::Strategy`].
//! - [`agent`] — wraps a `Strategy` as a `dominion_engine::Agent`.
//! - [`kingdom`] — resolves `--kingdom` CLI arguments (presets / explicit lists / `auto`).
//! - [`stats`] — Wilson confidence intervals and per-strategy accumulators.
//! - [`batch`] *(feature `parallel`)* — the rayon-based batch runner (`match` / `league`).
//!
//! Everything except `batch` compiles for `wasm32-unknown-unknown` with `--no-default-features`
//! (no threads, no CLI parsing needed there — only strategy loading and the `Agent` impl).

pub mod agent;
pub mod expr;
pub mod kingdom;
pub mod stats;
pub mod strategy;

#[cfg(feature = "parallel")]
pub mod batch;

pub use agent::StrategyAgent;
pub use strategy::Strategy;
