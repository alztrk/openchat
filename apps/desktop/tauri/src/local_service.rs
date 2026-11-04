//! Owns the packaged local-service process and mediates renderer access to it.

pub mod commands;
pub mod dialogs;
pub mod process;
pub mod process_interrupt;
pub mod rpc_frame;
pub mod types;
pub mod writer;

#[cfg(test)]
mod tests;

pub(crate) use commands::*;
pub(crate) use types::*;
