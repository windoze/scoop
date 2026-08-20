//! Native source configuration; filesystem checks follow target selection.

use crate::{ConditionalSourcePath, ConeRelativePath};

mod flags;
mod language;
mod libraries;
mod parse;

pub use flags::{NativeCompileFlag, NativeIncludeFlag};
pub use language::NativeSourceLanguage;
pub use libraries::NativeLibrary;
pub(crate) use parse::RawNativeConfig;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeConfig {
    sources: Vec<ConditionalSourcePath>,
    include: Vec<ConeRelativePath>,
    c_flags: Vec<NativeCompileFlag>,
    cxx_flags: Vec<NativeCompileFlag>,
    cxx: bool,
    libraries: Vec<NativeLibrary>,
}

impl NativeConfig {
    pub fn sources(&self) -> &[ConditionalSourcePath] {
        &self.sources
    }

    pub fn include(&self) -> &[ConeRelativePath] {
        &self.include
    }

    pub fn c_flags(&self) -> &[NativeCompileFlag] {
        &self.c_flags
    }

    pub fn cxx_flags(&self) -> &[NativeCompileFlag] {
        &self.cxx_flags
    }

    pub fn cxx(&self) -> bool {
        self.cxx
    }
}

#[cfg(test)]
mod tests;
