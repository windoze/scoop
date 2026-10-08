//! Artifact-only program linking and its ordinary runtime object inputs.
mod artifacts;
mod dynamic;
mod final_image;
mod link;
mod macho_cursor;
mod macho_exports;
mod namespace;
mod native_input;
mod native_object;
mod program;
mod runtime;
mod startup;

#[cfg(test)]
mod test_support;

pub use artifacts::{ArtifactLinkRequest, read_program_artifacts, resolve_program_link_profile};
pub use link::{ProgramLinkOutput, ResolvedLinkPlanFingerprint, link_program};
pub use native_object::{NativeObjectInfo, NativeSymbolDefinition, NativeSymbolKind};
pub use runtime::*;

#[derive(Debug)]
pub struct LinkError(pub String);
impl std::fmt::Display for LinkError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for LinkError {}

pub(crate) fn error(value: impl std::fmt::Display) -> LinkError {
    LinkError(value.to_string())
}
