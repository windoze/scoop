//! Single-file dependencies in the initial source trivia, before any language token.
use std::collections::BTreeSet;
use std::fmt;
use std::ops::Range;

use scoop_identity::ConeCoordinate;
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SingleFileDependency {
    #[serde(with = "crate::serde_record::coordinate")]
    coordinate: ConeCoordinate,
    span: Option<Range<usize>>,
}

impl SingleFileDependency {
    pub fn coordinate(&self) -> &ConeCoordinate {
        &self.coordinate
    }
    pub fn span(&self) -> Option<Range<usize>> {
        self.span.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SingleFileHeaderError {
    pub span: Range<usize>,
    pub message: String,
}

impl fmt::Display for SingleFileHeaderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for SingleFileHeaderError {}

pub fn parse_single_file_dependencies(
    source: &str,
) -> Result<Vec<SingleFileDependency>, SingleFileHeaderError> {
    let mut dependencies = [
        ConeCoordinate::reserved_core(),
        ConeCoordinate::new("scoop", "scoop.std", "0.1.0")
            .expect("the standard library coordinate is valid"),
    ]
    .into_iter()
    .map(|coordinate| SingleFileDependency {
        coordinate,
        span: None,
    })
    .collect::<Vec<_>>();
    let mut explicit = BTreeSet::new();
    let mut offset = if source.starts_with("#!") {
        line_end(source, 0)
    } else {
        0
    };
    while offset < source.len() {
        let rest = &source[offset..];
        if matches!(rest.as_bytes()[0], b' ' | b'\t' | b'\r' | b'\n') {
            offset += 1;
        } else if rest.starts_with("//") {
            let end = line_end(source, offset);
            let comment_end = if source[..end].ends_with('\r') {
                end - 1
            } else {
                end
            };
            if let Some(directive) = source[offset..comment_end].strip_prefix("//!") {
                let text = directive.trim();
                if let Some(tail) = text.strip_prefix("dep")
                    && (tail.is_empty()
                        || tail.starts_with(':')
                        || tail.starts_with(char::is_whitespace))
                {
                    let span = offset..comment_end;
                    let error = |message: String| SingleFileHeaderError {
                        span: span.clone(),
                        message,
                    };
                    let coordinate = parse_coordinate(tail).map_err(error)?;
                    if !explicit.insert(coordinate.to_string()) {
                        return Err(error(format!(
                            "duplicate single-file dependency `{coordinate}`"
                        )));
                    }
                    if let Some(existing) = dependencies
                        .iter_mut()
                        .find(|entry| entry.coordinate == coordinate)
                    {
                        existing.span = Some(span);
                    } else {
                        dependencies.push(SingleFileDependency {
                            coordinate,
                            span: Some(span),
                        });
                    }
                }
            }
            offset = end;
        } else if let Some(body) = rest.strip_prefix("/*") {
            // Match ordinary non-nesting block-comment syntax. The parser diagnoses an unclosed comment.
            let Some(end) = body.find("*/") else {
                break;
            };
            offset += end + 4;
        } else {
            break;
        }
    }
    Ok(dependencies)
}

fn line_end(source: &str, start: usize) -> usize {
    source[start..]
        .find('\n')
        .map_or(source.len(), |end| start + end)
}

fn parse_coordinate(tail: &str) -> Result<ConeCoordinate, String> {
    let text = tail
        .trim_start()
        .strip_prefix(':')
        .ok_or_else(|| "single-file dep directive requires `dep: group:name:version`".to_owned())?
        .trim();
    let parts = text.split(':').collect::<Vec<_>>();
    let [group, name, version] = parts.as_slice() else {
        return Err(
            "single-file dependency must be an exact `group:name:version` coordinate".into(),
        );
    };
    ConeCoordinate::new(group, name, version).map_err(|error| error.to_string())
}
