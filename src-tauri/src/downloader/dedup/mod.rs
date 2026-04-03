//! Asset de-duplication and storage optimization.
//!
//! This module provides a content-based de-duplication system. It maintains
//! an index of asset hashes to identify identical files across different
//! cases and promote them to a shared `defaults/shared/` directory.

mod helpers;
pub use helpers::*;

mod index;
pub use index::*;

mod operations;
pub use operations::*;

mod optimize;
pub use optimize::*;

#[cfg(test)]
mod tests;
