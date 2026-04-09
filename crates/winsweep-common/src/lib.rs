//! WinSweep Common Types and Utilities
//!
//! This crate contains shared types, constants, and utilities used across all WinSweep components.

pub mod config;
pub mod never_delete;
pub mod project_signatures;
pub mod types;

// Re-export commonly used types
pub use config::Config;
pub use never_delete::NEVER_DELETE_PATHS;
pub use project_signatures::ProjectSignature;
pub use types::ProjectType;
pub use types::*;
