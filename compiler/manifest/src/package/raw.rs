use std::collections::BTreeMap;

use serde::Deserialize;
use toml::Spanned;

pub(super) type DependencyMap = BTreeMap<String, Spanned<toml::Value>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawPackage {
    pub schema: Spanned<i64>,
    pub cone: Option<Spanned<RawCone>>,
    pub workspace: Option<RawWorkspace>,
    #[serde(default)]
    pub registry: RawRegistry,
    #[serde(default)]
    pub dependencies: DependencyMap,
    #[serde(default, rename = "test-dependencies")]
    pub test_dependencies: DependencyMap,
    #[serde(default)]
    pub overrides: DependencyMap,
    pub sources: Option<Vec<crate::selection::RawConditionalPath>>,
    #[serde(default)]
    pub test: RawTest,
    #[serde(default)]
    pub native: crate::native::RawNativeConfig,
    pub windows: Option<Spanned<crate::windows::RawWindowsConfig>>,
    #[serde(default)]
    pub publish: RawPublish,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawCone {
    pub group: Spanned<toml::Value>,
    pub name: Spanned<String>,
    pub version: Spanned<toml::Value>,
    pub kind: Spanned<String>,
    pub license: Option<Spanned<toml::Value>>,
    pub repository: Option<Spanned<toml::Value>>,
    pub description: Option<Spanned<String>>,
    pub readme: Option<Spanned<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawWorkspace {
    #[serde(default)]
    pub members: Vec<Spanned<String>>,
    #[serde(rename = "default-members")]
    pub default_members: Option<Vec<Spanned<String>>>,
    #[serde(default)]
    pub cone: BTreeMap<String, Spanned<String>>,
    #[serde(default)]
    pub dependencies: DependencyMap,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct RawRegistry {
    pub default: Option<Spanned<String>>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct RawTest {
    pub sources: Option<Vec<crate::selection::RawConditionalPath>>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct RawPublish {
    pub include: Option<Vec<Spanned<String>>>,
    #[serde(default)]
    pub exclude: Vec<Spanned<String>>,
    #[serde(default)]
    pub files: Vec<RawPublishFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawPublishFile {
    pub from: Spanned<String>,
    pub to: Spanned<String>,
}
