//! Native source configuration; filesystem checks follow target selection.

use crate::ConditionalSourcePath;
use scoop_identity::TargetProfileId;
use std::path::PathBuf;

mod flags;
mod include;
mod language;
mod libraries;
mod parse;

pub use flags::{NativeCompileFlag, NativeIncludeFlag};
pub use language::NativeSourceLanguage;
pub use libraries::NativeLibrary;
pub(crate) use parse::RawNativeConfig;

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NativeConfig {
    sources: Vec<ConditionalSourcePath>,
    include: Vec<include::ConditionalInclude>,
    c_flags: Vec<NativeCompileFlag>,
    cxx_flags: Vec<NativeCompileFlag>,
    objc_flags: Vec<NativeCompileFlag>,
    cxx: bool,
    libraries: Vec<NativeLibrary>,
}

impl NativeConfig {
    pub fn sources(&self) -> &[ConditionalSourcePath] {
        &self.sources
    }

    pub fn include_paths(&self, target: TargetProfileId) -> impl Iterator<Item = &PathBuf> {
        self.include
            .iter()
            .filter(move |entry| entry.predicate.matches(target))
            .map(|entry| &entry.path)
    }

    pub fn c_flags(&self) -> &[NativeCompileFlag] {
        &self.c_flags
    }

    pub fn cxx_flags(&self) -> &[NativeCompileFlag] {
        &self.cxx_flags
    }

    pub fn objc_flags(&self) -> &[NativeCompileFlag] {
        &self.objc_flags
    }

    pub fn cxx(&self) -> bool {
        self.cxx
    }
}

#[cfg(test)]
mod tests;
