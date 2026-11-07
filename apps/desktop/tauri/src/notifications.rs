//! Bridges validated operating-system notification activations to the frontend.
//!
//! Activations arriving before frontend readiness are held in a bounded queue. Identifiers,
//! reply text, and notification copy are validated before dispatch or native delivery.

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod activation;
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod commands;

#[cfg(target_os = "linux")]
mod linux_backend;
#[cfg(target_os = "windows")]
mod windows_backend;

#[cfg(test)]
mod tests;

pub use activation::*;
pub use commands::*;

#[cfg(target_os = "windows")]
pub(crate) fn set_process_app_user_model_id() -> Result<(), String> {
    windows_backend::set_process_app_user_model_id()
}
