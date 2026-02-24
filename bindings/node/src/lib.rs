#![deny(clippy::all)]

pub mod ask;
pub mod error;
pub mod frame;
pub mod memory;
pub mod memvid;
pub mod mesh;
pub mod schema;
pub mod payload;
pub mod search;
pub mod sketch;
pub mod stream;
pub mod timeline;
pub mod write;

use napi_derive::napi;

/// Returns the memvid-core version string.
#[napi]
pub fn version() -> String {
    memvid_core::MEMVID_CORE_VERSION.to_string()
}
