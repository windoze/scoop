//! Source-condition facts supplied by the caller's supported target catalog.

use scoop_identity::TargetProfileId;

#[derive(Clone, Copy, Debug)]
pub struct SourceTarget<'a> {
    pub id: TargetProfileId,
    pub triple: &'a str,
    pub os: &'a str,
    pub arch: &'a str,
    pub env: &'a str,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub(super) struct SelectedSourceTarget {
    #[serde(with = "crate::serde_record::target")]
    pub(super) id: TargetProfileId,
    pub(super) triple: String,
}
