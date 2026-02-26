//! VM management module for bisque-computer.
//!
//! Provides filesystem isolation, virtio-fs share configuration, the drop
//! folder HTTP server and FSEvents watcher for ingesting files into the
//! Lobster VM sandbox, and the remote communication channel.

pub mod drop_server;
pub mod filesystem;
pub mod remote_channel;
