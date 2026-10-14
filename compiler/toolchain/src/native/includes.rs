//! Candidate inputs from the compiler's expanded include directives (`-dI`).
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Only raw strings can span lines in the compiler's preprocessed token stream.
#[derive(Default)]
pub(super) struct RawString(Option<Vec<u8>>);

impl RawString {
    pub(super) fn contains_start_of_line(&mut self, mut line: &[u8]) -> bool {
        let inside = self.0.is_some();
        while !line.is_empty() {
            if let Some(end) = &self.0 {
                let Some(index) = line.windows(end.len()).position(|v| v == end) else {
                    break;
                };
                line = &line[index + end.len()..];
                self.0 = None;
            } else if line.starts_with(b"R\"") {
                let Some(index) = line.iter().position(|b| *b == b'(') else {
                    break;
                };
                self.0 = Some([b")".as_slice(), &line[2..index], b"\""].concat());
                line = &line[index + 1..];
            } else if matches!(line[0], b'\"' | b'\'') {
                let quote = line[0];
                line = &line[1..];
                while let Some(&byte) = line.first() {
                    line = &line[1..];
                    if byte == quote {
                        break;
                    }
                    if byte == b'\\' && !line.is_empty() {
                        line = &line[1..];
                    }
                }
            } else if line[0].is_ascii_digit() {
                // C++ digit separators belong to the numeric token, not a char literal.
                let end = line
                    .iter()
                    .position(|b| !b.is_ascii_alphanumeric() && !matches!(b, b'.' | b'_' | b'\''))
                    .unwrap_or(line.len());
                line = &line[end..];
            } else {
                line = &line[1..];
            }
        }
        inside
    }
}

pub(super) fn observe(
    line: &[u8],
    source: &Path,
    roots: &[PathBuf],
    inputs: &mut BTreeSet<PathBuf>,
) -> bool {
    let Some(include) = [
        b"#include ".as_slice(),
        b"#include_next ",
        b"#import ",
        b"#__include_macros ",
    ]
    .iter()
    .find_map(|prefix| line.strip_prefix(*prefix)) else {
        return false;
    };
    let quote = include.first() == Some(&b'"');
    let end = if quote { b'"' } else { b'>' };
    let Some(name) = include.get(1..).and_then(|v| v.split(|b| *b == end).next()) else {
        return false;
    };
    // Native paths already use UTF-8 for compiler diagnostics and line markers.
    let Ok(name) = std::str::from_utf8(name) else {
        return false;
    };
    if quote && let Some(parent) = source.parent() {
        inputs.insert(parent.join(name));
    }
    for root in roots {
        inputs.insert(root.join(name));
        if root.file_name().is_some_and(|n| n == "Frameworks")
            && let Some((framework, header)) = name.split_once('/')
        {
            for kind in ["Headers", "PrivateHeaders"] {
                inputs.insert(root.join(format!("{framework}.framework/{kind}/{header}")));
            }
        }
    }
    true
}
