use std::path::Path;

/// A lexical Cone-relative input path; `.` denotes the Cone directory itself.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct ConeRelativePath(String);

impl ConeRelativePath {
    pub fn new(value: &str) -> Result<Self, &'static str> {
        Self::normalize(value, false)
    }

    pub(crate) fn native(value: &str) -> Result<Self, &'static str> {
        Self::normalize(value, true)
    }

    fn normalize(value: &str, allow_parent: bool) -> Result<Self, &'static str> {
        if value.is_empty() {
            return Err("path must not be empty");
        }
        if value.starts_with('/') || has_drive_prefix(value) {
            return Err("path must be relative to the Cone root");
        }
        if value.contains(['\\', '\0', '*', '?', '[', ']']) {
            return Err("path must not contain backslashes, NUL, or glob metacharacters");
        }
        let mut segments = Vec::new();
        for segment in value.split('/') {
            match segment {
                "" | "." => {}
                ".." => {
                    if segments.last().is_some_and(|part| *part != "..") {
                        segments.pop();
                    } else if allow_parent {
                        segments.push("..");
                    } else {
                        return Err("path must not escape the Cone root");
                    }
                }
                _ => segments.push(segment),
            }
        }
        let normalized = if segments.is_empty() {
            ".".to_owned()
        } else {
            segments.join("/")
        };
        if has_drive_prefix(&normalized) {
            return Err("path must be relative to the Cone root");
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn as_path(&self) -> &Path {
        Path::new(if self.0 == "." { "" } else { &self.0 })
    }
}

fn has_drive_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}
