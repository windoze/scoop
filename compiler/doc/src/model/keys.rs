use scoop_hir::{AnnotationTargetV1, SourceNominalId};
use scoop_identity::{ConeIdentity, PersistentObjectValueId, SourceIdentity};
use scoop_wire::{HashError, WireEncode, domain_separated_cbor_hash};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ConeKey(pub String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SourceKey(pub String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SymbolKey(pub String);

fn key(domain: &str, value: &impl WireEncode) -> Result<String, HashError> {
    domain_separated_cbor_hash(domain, value).map(|digest| digest.to_string())
}

impl ConeKey {
    pub fn new(cone: ConeIdentity) -> Result<Self, HashError> {
        key("scoop-doc.cone", &cone).map(Self)
    }
}

impl SourceKey {
    pub fn new(source: &SourceIdentity) -> Result<Self, HashError> {
        key("scoop-doc.source", source).map(Self)
    }
}

impl SymbolKey {
    pub fn declaration(target: AnnotationTargetV1) -> Result<Self, HashError> {
        key("scoop-doc.declaration", &target).map(|key| Self(format!("d-{key}")))
    }

    pub fn derived_equality(owner: SourceNominalId) -> Result<Self, HashError> {
        key("scoop-doc.derived-equality", &owner).map(|key| Self(format!("e-{key}")))
    }

    pub fn object_value(value: PersistentObjectValueId) -> Result<Self, HashError> {
        key("scoop-doc.object-value", &value).map(|key| Self(format!("o-{key}")))
    }

    pub(crate) fn child(&self, role: &str, position: u32) -> Self {
        Self(format!("{}-{role}-{position}", self.0))
    }
}
