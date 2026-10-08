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
) -> Result<SourceSelection, ManifestParseError> {
    match raw {
        None => Ok(SourceSelection::Default),
        Some(raw) => raw
            .into_iter()
            .map(parse_path)
            .collect::<Result<Vec<_>, _>>()
            .map(SourceSelection::Explicit),
    }
}

pub(crate) fn parse_path(
    raw: RawConditionalPath,
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
    let mut targets = TargetProfileId::ALL.to_vec();
    for (key, value) in raw.when.into_iter().flatten() {
        let error = |reason: String| {
            ManifestParseError::new(
                ManifestParseErrorKind::InvalidSelection(reason),
                Some(value.span()),
            )
        };
        let field: fn(TargetProfileId) -> &'static str = match key.as_str() {
            "os" => TargetProfileId::os,
            "arch" => TargetProfileId::arch,
            "env" => TargetProfileId::env,
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
            if !TargetProfileId::ALL
                .into_iter()
                .any(|target| field(target) == *value)
            {
                return Err(error(format!(
                    "unknown target predicate {key:?} value {value:?}"
                )));
            }
        }
        targets.retain(|target| values.contains(&field(*target)));
    }
    Ok(ConditionalSourcePath {
        path,
        predicate: TargetPredicate { targets },
    })
}
