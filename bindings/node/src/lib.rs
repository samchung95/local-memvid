#![deny(clippy::all)]

pub mod error;
pub mod memvid;
pub mod write;

use napi_derive::napi;

/// Returns the memvid-core version string.
#[napi]
pub fn version() -> String {
    memvid_core::MEMVID_CORE_VERSION.to_string()
}
