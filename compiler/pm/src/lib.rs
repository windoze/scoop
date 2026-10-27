//! Workspace and package management, separate from the compiler and build driver.

pub mod cache;
mod error;
pub mod lockfile;
mod workspace;

pub use error::{ManifestOrigin, PmError, PmResult};
pub use workspace::{
    ExpandedCone, ExpandedDependency, MemberSelection, Workspace, WorkspaceMember,
};
