use scoop_wire::{Encoder, HashError, WireEncode};

use crate::ids::derive_persistent_id;
use crate::{
    ConeIdentity, DigestNodeId, ObjectDefinitionAtomId, PersistentCallableBodyId,
    PersistentLayoutId, PersistentSafepointSiteId, PersistentScanId,
};

mod decode;

pub use decode::{
    DecodedDigestNodeKey, DecodedDigestOwnerAndRoleKey, DigestNodeKeyResolutionError,
    DigestOwnerResolver,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DigestKind {
    SourceSignature,
    Layout,
    Scan,
    ObjectSupport,
    ObjectDefinition,
    StackmapRecord,
    RuntimeImage,
}

impl DigestKind {
    pub const fn tag(self) -> u32 {
        match self {
            Self::SourceSignature => 1,
            Self::Layout => 2,
            Self::Scan => 3,
            Self::ObjectSupport => 5,
            Self::ObjectDefinition => 6,
            Self::StackmapRecord => 7,
            Self::RuntimeImage => 10,
        }
    }
}

impl WireEncode for DigestKind {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.tag()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DigestOwnerAndRoleKey {
    SourceSignature(PersistentCallableBodyId),
    Layout(PersistentLayoutId),
    Scan(PersistentScanId),
    ObjectSupport(ObjectDefinitionAtomId),
    ObjectDefinition(ObjectDefinitionAtomId),
    StackmapRecord(PersistentSafepointSiteId),
    RuntimeImage(ConeIdentity),
}

impl DigestOwnerAndRoleKey {
    pub const fn kind(self) -> DigestKind {
        match self {
            Self::SourceSignature(_) => DigestKind::SourceSignature,
            Self::Layout(_) => DigestKind::Layout,
            Self::Scan(_) => DigestKind::Scan,
            Self::ObjectSupport(_) => DigestKind::ObjectSupport,
            Self::ObjectDefinition(_) => DigestKind::ObjectDefinition,
            Self::StackmapRecord(_) => DigestKind::StackmapRecord,
            Self::RuntimeImage(_) => DigestKind::RuntimeImage,
        }
    }
}

impl WireEncode for DigestOwnerAndRoleKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::SourceSignature(id) => encode_value_sum(encoder, 1, id),
            Self::Layout(id) => encode_value_sum(encoder, 2, id),
            Self::Scan(id) => encode_value_sum(encoder, 3, id),
            Self::ObjectSupport(id) => encode_value_sum(encoder, 5, id),
            Self::ObjectDefinition(id) => encode_value_sum(encoder, 6, id),
            Self::StackmapRecord(id) => encode_value_sum(encoder, 7, id),
            Self::RuntimeImage(id) => encode_value_sum(encoder, 10, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DigestNodeKey {
    kind: DigestKind,
    owner_and_role: DigestOwnerAndRoleKey,
}

impl DigestNodeKey {
    pub const fn source_signature(owner: PersistentCallableBodyId) -> Self {
        Self::from_owner(DigestOwnerAndRoleKey::SourceSignature(owner))
    }

    pub const fn layout(owner: PersistentLayoutId) -> Self {
        Self::from_owner(DigestOwnerAndRoleKey::Layout(owner))
    }

    pub const fn scan(owner: PersistentScanId) -> Self {
        Self::from_owner(DigestOwnerAndRoleKey::Scan(owner))
    }

    pub const fn object_support(owner: ObjectDefinitionAtomId) -> Self {
        Self::from_owner(DigestOwnerAndRoleKey::ObjectSupport(owner))
    }

    pub const fn object_definition(owner: ObjectDefinitionAtomId) -> Self {
        Self::from_owner(DigestOwnerAndRoleKey::ObjectDefinition(owner))
    }

    pub const fn stackmap_record(owner: PersistentSafepointSiteId) -> Self {
        Self::from_owner(DigestOwnerAndRoleKey::StackmapRecord(owner))
    }

    pub const fn runtime_image(owner: ConeIdentity) -> Self {
        Self::from_owner(DigestOwnerAndRoleKey::RuntimeImage(owner))
    }

    const fn from_owner(owner_and_role: DigestOwnerAndRoleKey) -> Self {
        Self {
            kind: owner_and_role.kind(),
            owner_and_role,
        }
    }

    pub(crate) fn from_parts(
        kind: DigestKind,
        owner_and_role: DigestOwnerAndRoleKey,
    ) -> Result<Self, DigestNodeKeyError> {
        if kind != owner_and_role.kind() {
            return Err(DigestNodeKeyError::KindOwnerMismatch {
                kind,
                owner_kind: owner_and_role.kind(),
            });
        }
        Ok(Self {
            kind,
            owner_and_role,
        })
    }

    pub const fn kind(self) -> DigestKind {
        self.kind
    }

    pub const fn owner_and_role(self) -> DigestOwnerAndRoleKey {
        self.owner_and_role
    }
}

impl WireEncode for DigestNodeKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.kind.encode(encoder)?;
        encoder.field(2)?;
        self.owner_and_role.encode(encoder)
    }
}

impl DigestNodeId {
    pub fn from_key(key: &DigestNodeKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-digest-node-id-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DigestNodeKeyError {
    KindOwnerMismatch {
        kind: DigestKind,
        owner_kind: DigestKind,
    },
}

impl std::fmt::Display for DigestNodeKeyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("digest node kind does not match its typed owner-and-role key")
    }
}

impl std::error::Error for DigestNodeKeyError {}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{DigestKind, DigestNodeKey, DigestOwnerAndRoleKey};
    use crate::{ConeIdentity, DigestNodeId, PersistentCallableBodyId};

    #[test]
    fn digest_kinds_and_owners_keep_their_independent_wire_tags() {
        let raw = ConeIdentity::CORE.0;
        let keys = [
            DigestNodeKey::source_signature(PersistentCallableBodyId(raw)),
            DigestNodeKey::layout(crate::PersistentLayoutId(raw)),
            DigestNodeKey::scan(crate::PersistentScanId(raw)),
            DigestNodeKey::object_support(crate::ObjectDefinitionAtomId(raw)),
            DigestNodeKey::object_definition(crate::ObjectDefinitionAtomId(raw)),
            DigestNodeKey::stackmap_record(crate::PersistentSafepointSiteId(raw)),
            DigestNodeKey::runtime_image(ConeIdentity::CORE),
        ];

        for ((key, tag), owner_tag) in keys
            .into_iter()
            .zip([1_u8, 2, 3, 5, 6, 7, 10])
            .zip([1, 2, 3, 5, 6, 7, 10])
        {
            assert_eq!(key.kind().tag(), u32::from(tag));
            assert_eq!(key.owner_and_role().kind(), key.kind());
            let encoded = encode(&key).unwrap();
            assert_eq!(&encoded[..3], &[0xa2, 0x01, tag]);
            assert_eq!(encoded[6], owner_tag);
        }
    }

    #[test]
    fn retired_content_digest_tags_are_rejected() {
        use scoop_wire::decode_canonical;
        for tag in [4, 8, 9] {
            assert!(decode_canonical::<DigestKind>(&[tag]).is_err());
        }
        for tag in [4, 9, 11] {
            let mut bytes = vec![0xa2, 0, tag, 1, 0x58, 32];
            bytes.extend_from_slice(ConeIdentity::CORE.as_array());
            let error =
                decode_canonical::<super::DecodedDigestOwnerAndRoleKey>(&bytes).unwrap_err();
            assert_eq!(
                error.kind(),
                &scoop_wire::WireErrorKind::UnknownTag {
                    tag: u64::from(tag)
                }
            );
        }
    }

    #[test]
    fn node_identities_have_fixed_preimages() {
        let source_key =
            DigestNodeKey::source_signature(PersistentCallableBodyId(ConeIdentity::SINGLE_FILE.0));
        let source = DigestNodeId::from_key(&source_key).unwrap();
        assert_eq!(
            source.to_string(),
            "68ed239bb24f76f50d6c54b3cd54f985e787d8110f841b2009f2613d4025ce1e"
        );
        assert_eq!(
            source_key.owner_and_role(),
            DigestOwnerAndRoleKey::SourceSignature(PersistentCallableBodyId(
                ConeIdentity::SINGLE_FILE.0
            ))
        );
    }
}
