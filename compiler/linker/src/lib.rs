//! Artifact-only program linking and its ordinary runtime object inputs.
mod native_object;
mod runtime;

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
