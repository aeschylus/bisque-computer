//! VM management module for bisque-computer.
//!
//! Provides filesystem isolation, virtio-fs drop folder configuration, the
//! drop folder event type, and the remote communication channel for the
//! Lobster VM sandbox.
//!
//! ## Sub-modules
//!
//! - [`filesystem`] — virtio-fs argument building and virtual disk helpers
//! - [`drop_server`] — `DropEvent` type and file-ingestion helpers

pub mod drop_server;
pub mod filesystem;
