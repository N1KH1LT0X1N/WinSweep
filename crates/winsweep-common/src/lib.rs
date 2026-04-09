//! WinSweep Common Types and Utilities
//! 
//! This crate contains shared types, constants, and utilities used across all WinSweep components.

pub mod types;
pub mod config;
pub mod project_signatures;
pub mod never_delete;

// Re-export commonly used types
pub use types::*;
pub use config::Config;
pub use types::ProjectType;
pub use project_signatures::ProjectSignature;
pub use never_delete::NEVER_DELETE_PATHS;
