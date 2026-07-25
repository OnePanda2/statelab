//! Built-in deterministic systems.
//!
//! Each submodule implements [`crate::system::DeterministicSystem`] for exactly
//! one system. Classic Collatz is the **first** built-in system, not a special
//! case — the engine never references anything in here.

pub mod collatz;
