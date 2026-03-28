//! Import cases from existing aaoffline downloads.
//!
//! Supports importing from the aaoffline format:
//!   source_dir/
//!   ├── index.html      (contains trial_information + initial_trial_data as inline JS)
//!   └── assets/          (all case assets with hash-suffixed filenames)
//!
//! The import:
//! 1. Parses trial_information and initial_trial_data from the inlined JS
//! 2. Rewrites asset paths from "assets/..." to "case/{id}/assets/..."
//! 3. Copies the assets/ directory
//! 4. Generates manifest.json, trial_info.json, trial_data.json

mod shared;
pub use shared::*;

pub(crate) mod aaoffline_helpers;

mod aaoffline;
pub use aaoffline::*;

mod saves;
pub use saves::*;

mod plugins_utils;
pub use plugins_utils::*;

mod plugins_global;
pub use plugins_global::*;

mod plugins_case;
pub use plugins_case::*;

mod case_import;
pub use case_import::*;

mod case_export;
pub use case_export::*;

#[cfg(test)]
mod tests;
