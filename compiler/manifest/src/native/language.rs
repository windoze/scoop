use super::{NativeCompileFlag, NativeConfig};
use crate::ConeRelativePath;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum NativeSourceLanguage {
    C,
    Cxx,
}

impl NativeSourceLanguage {
    pub const fn input_name(self) -> &'static str {
        match self {
            Self::C => "c",
            Self::Cxx => "c++",
        }
    }

    pub const fn preprocessed_name(self) -> &'static str {
        match self {
            Self::C => "cpp-output",
            Self::Cxx => "c++-cpp-output",
        }
    }
}

impl NativeConfig {
    pub fn source_language(&self, path: &ConeRelativePath) -> Result<NativeSourceLanguage, String> {
        match path
            .as_path()
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some("c") => Ok(NativeSourceLanguage::C),
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
        }
    }
}
