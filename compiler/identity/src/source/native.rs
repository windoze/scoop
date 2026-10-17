use super::{NormalizedSourcePathError, validate_source_path};

/// A native translation-unit key, relative to its Cone, including shared inputs.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NormalizedNativeSourcePath(String);

impl NormalizedNativeSourcePath {
    pub fn new(value: &str) -> Result<Self, NormalizedSourcePathError> {
        validate_source_path(value, true)?;
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_keys_accept_normalized_parent_paths_only() {
        for path in ["native/answer.c", "../answer.c", "../../vendor/answer.cpp"] {
            assert_eq!(
                NormalizedNativeSourcePath::new(path).unwrap().as_str(),
                path
            );
        }
        for path in [
            "",
            "/answer.c",
            "C:/answer.c",
            "./answer.c",
            "a/../answer.c",
            "../a/../../b.c",
            "../a//b.c",
        ] {
            assert!(NormalizedNativeSourcePath::new(path).is_err(), "{path}");
        }
    }
}
