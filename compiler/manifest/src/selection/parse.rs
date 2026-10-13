use std::collections::BTreeMap;

use serde::Deserialize;
use toml::Spanned;

use super::*;
use crate::{ManifestParseError, ManifestParseErrorKind};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawConditionalPath {
    path: Spanned<String>,
    when: Option<BTreeMap<String, Spanned<toml::Value>>>,
}

pub(crate) fn parse_sources(
    raw: Option<Vec<RawConditionalPath>>,
    catalog: &[SourceTarget<'_>],
) -> Result<SourceSelection, ManifestParseError> {
    match raw {
        None => Ok(SourceSelection::Default),
        Some(raw) => raw
            .into_iter()
            .map(|raw| parse_path(raw, catalog))
            .collect::<Result<Vec<_>, _>>()
            .map(SourceSelection::Explicit),
    }
}

pub(crate) fn parse_path(
    raw: RawConditionalPath,
    catalog: &[SourceTarget<'_>],
) -> Result<ConditionalSourcePath, ManifestParseError> {
    let path = ConeRelativePath::new(raw.path.get_ref()).map_err(|reason| {
        ManifestParseError::new(
            ManifestParseErrorKind::InvalidSelection(format!(
                "invalid source path {:?}: {reason}",
                raw.path.get_ref()
            )),
            Some(raw.path.span()),
        )
    })?;
    Ok(ConditionalSourcePath {
        path,
        predicate: parse_predicate(raw.when, catalog)?,
    })
}

pub(crate) fn parse_predicate(
    when: Option<BTreeMap<String, Spanned<toml::Value>>>,
    catalog: &[SourceTarget<'_>],
) -> Result<TargetPredicate, ManifestParseError> {
    let mut targets: Vec<_> = catalog.iter().collect();
    for (key, value) in when.into_iter().flatten() {
        let error = |reason: String| {
            ManifestParseError::new(
                ManifestParseErrorKind::InvalidSelection(reason),
                Some(value.span()),
            )
        };
        let field: for<'a> fn(&SourceTarget<'a>) -> &'a str = match key.as_str() {
            "os" => |target| target.os,
            "arch" => |target| target.arch,
            "env" => |target| target.env,
            _ => return Err(error(format!("unknown target predicate key {key:?}"))),
        };
        let values = match value.get_ref() {
            toml::Value::String(value) => vec![value.as_str()],
            toml::Value::Array(values) => values
                .iter()
                .map(|value| value.as_str())
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| error(format!("target predicate {key:?} requires strings")))?,
            _ => {
                return Err(error(format!(
                    "target predicate {key:?} requires a string or an array of strings"
                )));
            }
        };
        for value in &values {
            if !catalog.iter().any(|target| field(target) == *value) {
                return Err(error(format!(
                    "unknown target predicate {key:?} value {value:?}"
                )));
            }
        }
        targets.retain(|target| values.contains(&field(target)));
    }
    targets.sort_by(|a, b| a.triple.cmp(b.triple));
    Ok(TargetPredicate {
        targets: targets
            .into_iter()
            .map(|target| SelectedSourceTarget {
                id: target.id,
                triple: target.triple.to_owned(),
            })
            .collect(),
    })
}
