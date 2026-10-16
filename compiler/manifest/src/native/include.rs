use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;
use toml::Spanned;

use crate::{ManifestParseError, ManifestParseErrorKind, SourceTarget, TargetPredicate};

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct ConditionalInclude {
    pub path: PathBuf,
    pub predicate: TargetPredicate,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum RawInclude {
    Path(String),
    Conditional {
        path: String,
        when: Option<BTreeMap<String, toml::Value>>,
        #[serde(flatten)]
        extra: BTreeMap<String, toml::Value>,
    },
}

pub(super) fn parse(
    raw: Spanned<RawInclude>,
    catalog: &[SourceTarget<'_>],
) -> Result<ConditionalInclude, ManifestParseError> {
    let span = raw.span();
    let invalid = |reason: String| {
        ManifestParseError::new(
            ManifestParseErrorKind::InvalidNative(reason),
            Some(span.clone()),
        )
    };
    let (path, when) = match raw.into_inner() {
        RawInclude::Path(path) => (path, None),
        RawInclude::Conditional { path, when, extra } => {
            if let Some(field) = extra.keys().next() {
                return Err(invalid(format!("unknown native include field {field:?}")));
            }
            (path, when)
        }
    };
    Ok(ConditionalInclude {
        path: super::parse::native_path(&path)
            .map_err(|reason| invalid(format!("invalid native option {path:?}: {reason}")))?,
        predicate: crate::selection::parse_predicate(
            when.map(|entries| {
                entries
                    .into_iter()
                    .map(|(key, value)| (key, Spanned::new(span.clone(), value)))
                    .collect()
            }),
            catalog,
        )?,
    })
}

#[cfg(test)]
mod tests {
    use crate::test_catalog::parse_cone_manifest;
    use scoop_target_data::{DARWIN_AARCH64_ID, LINUX_X86_64_GNU_ID, LINUX_X86_64_MUSL_ID};

    const CONE: &str = "schema = 1\n[cone]\ngroup = 'native.test'\nname = 'includes'\nversion = '1.0.0'\nkind = 'library'\n[native]\n";

    #[test]
    fn selects_target_headers_in_declared_order() {
        let manifest = parse_cone_manifest(&format!("{CONE}include = ['common', {{path = '../darwin', when = {{os = 'darwin'}}}}, {{path = '/gnu', when = {{env = 'gnu'}}}}, {{path = 'musl', when = {{env = 'musl'}}}}, {{path = 'last'}}]")).unwrap();
        for (target, expected) in [
            (DARWIN_AARCH64_ID, ["common", "../darwin", "last"]),
            (LINUX_X86_64_GNU_ID, ["common", "/gnu", "last"]),
            (LINUX_X86_64_MUSL_ID, ["common", "musl", "last"]),
        ] {
            assert_eq!(
                manifest
                    .semantic()
                    .native()
                    .include_paths(target)
                    .map(|p| p.to_str().unwrap())
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn rejects_invalid_entries_before_selection() {
        for entry in [
            "{path = '', when = {os = 'linux'}}",
            "{path = 'headers', when = {env = 'invalid'}}",
            "{path = 'headers', extra = true}",
            "{when = {os = 'linux'}}",
            "42",
        ] {
            assert!(
                parse_cone_manifest(&format!("{CONE}include = [{entry}]")).is_err(),
                "{entry}"
            );
        }
    }
}
