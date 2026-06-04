//! BLXCode plugin package registry and capability plumbing.
//!
//! v1 plugins are declarative packages. They can contribute metadata and
//! runtime command detectors, but BLXCode does not execute plugin-provided
//! code.

pub mod builtins;
pub mod commands;
pub mod install;
pub mod run_detectors;
pub mod store;
pub mod types;
