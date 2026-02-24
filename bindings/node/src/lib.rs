#![deny(clippy::all)]

use napi_derive::napi;

/// Returns the memvid-core version string.
#[napi]
pub fn version() -> String {
    memvid_core::MEMVID_CORE_VERSION.to_string()
}
