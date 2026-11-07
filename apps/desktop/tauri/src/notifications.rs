//! Bridges validated operating-system notification activations to the frontend.
//!
//! Activations arriving before frontend readiness are held in a bounded queue. Identifiers,
//! reply text, and notification copy are validated before dispatch or native delivery.

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod activation;
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod commands;

#[cfg(target_os = "windows")]
mod windows_backend;

#[cfg(test)]
mod tests;

pub use activation::*;
pub use commands::*;
