use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use toml::Spanned;

use crate::{ManifestParseError, ManifestParseErrorKind, SourceTarget, TargetPredicate};

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct ConditionalInclude {
    pub path: NativeIncludePath,
    pub predicate: TargetPredicate,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NativeIncludePath {
    Path(PathBuf),
    Sysroot(PathBuf),
}

impl NativeIncludePath {
    pub fn path(&self) -> &Path {
        match self {
            Self::Path(path) | Self::Sysroot(path) => path,
        }
    }

    pub fn resolve(&self, cone_root: &Path, target_root: &Path) -> PathBuf {
        match self {
            Self::Path(path) => cone_root.join(path),
            Self::Sysroot(path) => target_root.join(path),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum RawInclude {
    Path(String),
    Conditional {
        path: Option<String>,
        sysroot: Option<String>,
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
    let (path, sysroot, when) = match raw.into_inner() {
        RawInclude::Path(path) => (path, false, None),
        RawInclude::Conditional {
            path,
            sysroot,
            when,
            extra,
        } => {
            if let Some(field) = extra.keys().next() {
                return Err(invalid(format!("unknown native include field {field:?}")));
            }
            match (path, sysroot) {
                (Some(path), None) => (path, false, when),
                (None, Some(path)) => (path, true, when),
                _ => {
                    return Err(invalid(
                        "native include requires exactly one of path or sysroot".into(),
                    ));
                }
            }
        }
    };
    let path = super::parse::native_path(&path)
        .map_err(|reason| invalid(format!("invalid native option {path:?}: {reason}")))?;
    if sysroot
        && path.components().any(|part| {
            matches!(
                part,
                std::path::Component::RootDir | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(invalid("native sysroot include must be relative".into()));
    }
    Ok(ConditionalInclude {
        path: if sysroot {
            NativeIncludePath::Sysroot(path)
        } else {
            NativeIncludePath::Path(path)
        },
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
                    .includes(target)
                    .map(|p| p.path().to_str().unwrap())
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
            "{sysroot = ''}",
            "{sysroot = '/absolute'}",
            "{path = 'ordinary', sysroot = 'backend/include'}",
            "42",
        ] {
            assert!(
                parse_cone_manifest(&format!("{CONE}include = [{entry}]")).is_err(),
                "{entry}"
            );
        }
    }

    #[test]
    fn target_sysroot_paths_keep_their_kind_and_share_condition_ordering() {
        use std::path::Path;
        let manifest = parse_cone_manifest(&format!("{CONE}include = [{{sysroot = 'backend/include'}}, {{sysroot = 'linux/include', when = {{os = 'linux'}}}}, '../shared']")).unwrap();
        let config = manifest.semantic().native();
        assert!(matches!(
            config.includes(DARWIN_AARCH64_ID).next(),
            Some(super::NativeIncludePath::Sysroot(_))
        ));
        assert_eq!(
            config
                .include_paths(
                    DARWIN_AARCH64_ID,
                    Path::new("cone"),
                    Path::new("selected-target")
                )
                .collect::<Vec<_>>(),
            [
                Path::new("selected-target/backend/include"),
                Path::new("cone/../shared")
            ]
        );
        assert_eq!(
            config
                .include_paths(
                    LINUX_X86_64_GNU_ID,
                    Path::new("cone"),
                    Path::new("gnu-target")
                )
                .collect::<Vec<_>>(),
            [
                Path::new("gnu-target/backend/include"),
                Path::new("gnu-target/linux/include"),
                Path::new("cone/../shared")
            ]
        );
    }
}
