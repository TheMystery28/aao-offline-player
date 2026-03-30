mod download;
mod log;
mod url_encoding;
mod utils;
pub use download::download_assets;
// Used by test submodules via `use super::*`
#[cfg(test)]
use download::{download_single_asset, download_with_retry, PER_ASSET_TIMEOUT};
#[cfg(test)]
use log::DownloadLog;
#[cfg(test)]
use url_encoding::encode_url;
#[cfg(test)]
use utils::{check_skip_existing, generate_filename};

use serde::Serialize;

use super::manifest::FailedAsset;

#[derive(Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum DownloadEvent {
    #[serde(rename = "started")]
    Started { total: usize },
    #[serde(rename = "progress")]
    Progress {
        completed: usize,
        total: usize,
        current_url: String,
        bytes_downloaded: u64,
        elapsed_ms: u64,
    },
    #[serde(rename = "finished")]
    Finished {
        downloaded: usize,
        failed: usize,
        total_bytes: u64,
        #[serde(default)]
        dedup_saved_bytes: u64,
    },
    #[serde(rename = "error")]
    #[allow(dead_code)] // Part of JS API contract — download.js handles "error" events
    Error { message: String },
    #[serde(rename = "sequence_progress")]
    SequenceProgress {
        current_part: usize,
        total_parts: usize,
        part_title: String,
    },
}

#[derive(Debug, Clone)]
pub struct DownloadedAsset {
    pub original_url: String,
    pub local_path: String,
    pub size: u64,
    /// xxh3_64 hash of the file content, computed at download time from bytes in memory.
    pub content_hash: u64,
}

/// Result of a batch download operation.
pub struct DownloadResult {
    pub downloaded: Vec<DownloadedAsset>,
    pub failed: Vec<FailedAsset>,
}

#[cfg(test)]
mod test_url_encoding;
#[cfg(test)]
mod test_utils;
#[cfg(test)]
mod test_download;
