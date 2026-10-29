use super::{NativeCompileFlag, NativeConfig};
use crate::{ConeRelativePath, SourceTarget};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum NativeSourceLanguage {
    C,
    Cxx,
    ObjectiveC,
}

impl NativeSourceLanguage {
    pub const fn input_name(self) -> &'static str {
        match self {
            Self::C => "c",
            Self::Cxx => "c++",
            Self::ObjectiveC => "objective-c",
        }
    }

    pub const fn preprocessed_name(self) -> &'static str {
        match self {
            Self::C => "cpp-output",
            Self::Cxx => "c++-cpp-output",
            Self::ObjectiveC => "objective-c-cpp-output",
        }
    }
}

impl NativeConfig {
    pub fn source_language(
        &self,
        path: &ConeRelativePath,
        target: SourceTarget<'_>,
    ) -> Result<NativeSourceLanguage, String> {
        match path
            .as_path()
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some("c") => Ok(NativeSourceLanguage::C),
            Some("m") if target.os == "darwin" => Ok(NativeSourceLanguage::ObjectiveC),
            Some("m") => Err(format!(
                "{}: Objective-C native sources require a Darwin target",
                path.as_str()
            )),
            Some("cc" | "cpp" | "cxx" | "C") if self.cxx => Ok(NativeSourceLanguage::Cxx),
            Some("cc" | "cpp" | "cxx" | "C") => {
                Err(format!("{} requires native.cxx = true", path.as_str()))
            }
            _ => Err(format!(
                "{} has an invalid native source extension",
                path.as_str()
            )),
        }
    }

    pub fn flags(&self, language: NativeSourceLanguage) -> &[NativeCompileFlag] {
        match language {
            NativeSourceLanguage::C => &self.c_flags,
            NativeSourceLanguage::Cxx => &self.cxx_flags,
            NativeSourceLanguage::ObjectiveC => &self.objc_flags,
        }
    }
}
