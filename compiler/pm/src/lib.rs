//! Workspace and package management, separate from the compiler and build driver.

mod error;
mod workspace;

pub use error::{ManifestOrigin, PmError, PmResult};
pub use workspace::{
    ExpandedCone, ExpandedDependency, MemberSelection, Workspace, WorkspaceMember,
};
