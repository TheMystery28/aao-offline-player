//! Case and plugin import/export logic.
//!
//! This module handles importing cases from various formats (`.aaocase` ZIPs, 
//! `aaoffline` directories) and exporting them back to ZIPs for sharing.
//! It also manages the installation and scoping of AAO player plugins.

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
