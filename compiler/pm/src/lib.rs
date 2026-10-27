//! Workspace and package management, separate from the compiler and build driver.

pub mod cache;
pub mod catalog;
mod error;
pub mod lockfile;
pub mod prepare;
pub mod resolve;
pub mod resolver;
pub mod sources;
mod workspace;

pub use error::{ManifestOrigin, PmError, PmResult};
pub use workspace::{
    ExpandedCone, ExpandedDependency, MemberSelection, Workspace, WorkspaceMember,
};
