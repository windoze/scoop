use scoop_wire::{Encoder, HashError, WireEncode};

use super::DefinitionAtomRole;
use crate::ids::derive_persistent_id;
use crate::{
    ConeIdentity, DigestNodeId, DigestPatchIntentId, ObjectDefinitionAtomId,
    ObjectDefinitionPlanId, OdrGroupId, PersistentCallableBodyId, PersistentLayoutId,
    PersistentSafepointSiteId, PersistentScanId,
};

mod decode;

pub use decode::{
    DecodedDigestNodeKey, DecodedDigestOwnerAndRoleKey, DecodedDigestPatchIntentKey,
    DigestNodeKeyResolutionError, DigestOwnerResolver, DigestPatchIntentResolutionError,
    DigestPatchIntentResolver,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DigestKind {
    SourceSignature,
    Layout,
    Scan,
    LirDefinition,
    ObjectSupport,
    ObjectDefinition,
    StackmapRecord,
    OdrDefinition,
    StrongRegistration,
    RuntimeImage,
}

impl DigestKind {
    pub const fn tag(self) -> u32 {
        match self {
            Self::SourceSignature => 1,
            Self::Layout => 2,
            Self::Scan => 3,
            Self::LirDefinition => 4,
            Self::ObjectSupport => 5,
            Self::ObjectDefinition => 6,
            Self::StackmapRecord => 7,
            Self::OdrDefinition => 8,
            Self::StrongRegistration => 9,
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
    LirDefinition(ObjectDefinitionAtomId),
    ObjectSupport(ObjectDefinitionAtomId),
    ObjectDefinition(ObjectDefinitionAtomId),
    StackmapRecord(PersistentSafepointSiteId),
    OdrDefinition(OdrGroupId),
    StrongRegistration(ObjectDefinitionPlanId),
    RuntimeImage(ConeIdentity),
}

impl DigestOwnerAndRoleKey {
    pub const fn kind(self) -> DigestKind {
        match self {
            Self::SourceSignature(_) => DigestKind::SourceSignature,
            Self::Layout(_) => DigestKind::Layout,
            Self::Scan(_) => DigestKind::Scan,
            Self::LirDefinition(_) => DigestKind::LirDefinition,
            Self::ObjectSupport(_) => DigestKind::ObjectSupport,
            Self::ObjectDefinition(_) => DigestKind::ObjectDefinition,
            Self::StackmapRecord(_) => DigestKind::StackmapRecord,
            Self::OdrDefinition(_) => DigestKind::OdrDefinition,
            Self::StrongRegistration(_) => DigestKind::StrongRegistration,
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
            Self::LirDefinition(id) => encode_value_sum(encoder, 4, id),
            Self::ObjectSupport(id) => encode_value_sum(encoder, 5, id),
            Self::ObjectDefinition(id) => encode_value_sum(encoder, 6, id),
            Self::StackmapRecord(id) => encode_value_sum(encoder, 7, id),
            Self::OdrDefinition(id) => encode_value_sum(encoder, 8, id),
            Self::StrongRegistration(id) => encode_value_sum(encoder, 9, id),
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

    pub const fn lir_definition(owner: ObjectDefinitionAtomId) -> Self {
        Self::from_owner(DigestOwnerAndRoleKey::LirDefinition(owner))
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

    pub const fn odr_definition(owner: OdrGroupId) -> Self {
        Self::from_owner(DigestOwnerAndRoleKey::OdrDefinition(owner))
    }

    pub const fn strong_registration(owner: ObjectDefinitionPlanId) -> Self {
        Self::from_owner(DigestOwnerAndRoleKey::StrongRegistration(owner))
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DigestSemanticFieldRole {
    RegistrationDefinition,
    SourceSignature,
    Layout,
    Scan,
    DescriptorDefinition,
    CallableBodyDefinition,
    GatewayDefinition,
    NormalizedStackmap,
    RuntimeImage,
}

impl DigestSemanticFieldRole {
    pub const fn tag(self) -> u32 {
        match self {
            Self::RegistrationDefinition => 1,
            Self::SourceSignature => 2,
            Self::Layout => 3,
            Self::Scan => 4,
            Self::DescriptorDefinition => 5,
            Self::CallableBodyDefinition => 6,
            Self::GatewayDefinition => 7,
            Self::NormalizedStackmap => 8,
            Self::RuntimeImage => 9,
        }
    }

    pub const fn accepts_source(self, kind: DigestKind) -> bool {
        match self {
            Self::RegistrationDefinition => matches!(
                kind,
                DigestKind::StrongRegistration | DigestKind::OdrDefinition
            ),
            Self::SourceSignature => matches!(kind, DigestKind::SourceSignature),
            Self::Layout => matches!(kind, DigestKind::Layout),
            Self::Scan => matches!(kind, DigestKind::Scan),
            Self::DescriptorDefinition | Self::CallableBodyDefinition | Self::GatewayDefinition => {
                matches!(kind, DigestKind::ObjectDefinition)
            }
            Self::NormalizedStackmap => matches!(kind, DigestKind::StackmapRecord),
            Self::RuntimeImage => matches!(kind, DigestKind::RuntimeImage),
        }
    }
}

impl WireEncode for DigestSemanticFieldRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.tag()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DigestPatchIntentKey {
    source: DigestNodeId,
    target_definition: ObjectDefinitionPlanId,
    atom_role: DefinitionAtomRole,
    semantic_field_role: DigestSemanticFieldRole,
}

impl DigestPatchIntentKey {
    pub const fn new(
        source: DigestNodeId,
        target_definition: ObjectDefinitionPlanId,
        atom_role: DefinitionAtomRole,
        semantic_field_role: DigestSemanticFieldRole,
    ) -> Self {
        Self {
            source,
            target_definition,
            atom_role,
            semantic_field_role,
        }
    }

    pub const fn source(self) -> DigestNodeId {
        self.source
    }

    pub const fn target_definition(self) -> ObjectDefinitionPlanId {
        self.target_definition
    }

    pub const fn atom_role(self) -> DefinitionAtomRole {
        self.atom_role
    }

    pub const fn semantic_field_role(self) -> DigestSemanticFieldRole {
        self.semantic_field_role
    }
}

impl WireEncode for DigestPatchIntentKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.source.encode(encoder)?;
        encoder.field(2)?;
        self.target_definition.encode(encoder)?;
        encoder.field(3)?;
        self.atom_role.encode(encoder)?;
        encoder.field(4)?;
        self.semantic_field_role.encode(encoder)
    }
}

impl DigestPatchIntentId {
    pub fn from_key(key: &DigestPatchIntentKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-digest-patch-intent-id-v1", key)
    }
}

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

    use super::{
        DigestKind, DigestNodeKey, DigestOwnerAndRoleKey, DigestPatchIntentKey,
        DigestSemanticFieldRole,
    };
    use crate::{
        ConeIdentity, DefinitionAtomRole, DigestNodeId, DigestPatchIntentId,
        ObjectDefinitionPlanId, PersistentCallableBodyId,
    };

    #[test]
    fn digest_kinds_and_owners_share_frozen_tags() {
        let raw = ConeIdentity::CORE.0;
        let keys = [
            DigestNodeKey::source_signature(PersistentCallableBodyId(raw)),
            DigestNodeKey::layout(crate::PersistentLayoutId(raw)),
            DigestNodeKey::scan(crate::PersistentScanId(raw)),
            DigestNodeKey::lir_definition(crate::ObjectDefinitionAtomId(raw)),
            DigestNodeKey::object_support(crate::ObjectDefinitionAtomId(raw)),
            DigestNodeKey::object_definition(crate::ObjectDefinitionAtomId(raw)),
            DigestNodeKey::stackmap_record(crate::PersistentSafepointSiteId(raw)),
            DigestNodeKey::odr_definition(crate::OdrGroupId(raw)),
            DigestNodeKey::strong_registration(crate::ObjectDefinitionPlanId(raw)),
            DigestNodeKey::runtime_image(ConeIdentity::CORE),
        ];

        for (key, tag) in keys.into_iter().zip(1_u8..=10) {
            assert_eq!(key.kind().tag(), u32::from(tag));
            assert_eq!(key.owner_and_role().kind(), key.kind());
            let encoded = encode(&key).unwrap();
            assert_eq!(&encoded[..3], &[0xa2, 0x01, tag]);
            assert_eq!(encoded[6], tag);
        }
    }

    #[test]
    fn semantic_field_roles_close_the_source_kind_matrix() {
        use DigestKind as K;
        use DigestSemanticFieldRole as R;

        let cases = [
            (R::RegistrationDefinition, K::StrongRegistration),
            (R::RegistrationDefinition, K::OdrDefinition),
            (R::SourceSignature, K::SourceSignature),
            (R::Layout, K::Layout),
            (R::Scan, K::Scan),
            (R::DescriptorDefinition, K::ObjectDefinition),
            (R::CallableBodyDefinition, K::ObjectDefinition),
            (R::GatewayDefinition, K::ObjectDefinition),
            (R::NormalizedStackmap, K::StackmapRecord),
            (R::RuntimeImage, K::RuntimeImage),
        ];
        let all_kinds = [
            K::SourceSignature,
            K::Layout,
            K::Scan,
            K::LirDefinition,
            K::ObjectSupport,
            K::ObjectDefinition,
            K::StackmapRecord,
            K::OdrDefinition,
            K::StrongRegistration,
            K::RuntimeImage,
        ];

        for role in [
            R::RegistrationDefinition,
            R::SourceSignature,
            R::Layout,
            R::Scan,
            R::DescriptorDefinition,
            R::CallableBodyDefinition,
            R::GatewayDefinition,
            R::NormalizedStackmap,
            R::RuntimeImage,
        ] {
            for kind in all_kinds {
                assert_eq!(
                    role.accepts_source(kind),
                    cases.contains(&(role, kind)),
                    "unexpected source matrix result for {role:?} and {kind:?}"
                );
            }
        }
    }

    #[test]
    fn node_and_patch_identities_have_fixed_preimages() {
        let source_key =
            DigestNodeKey::source_signature(PersistentCallableBodyId(ConeIdentity::SINGLE_FILE.0));
        let source = DigestNodeId::from_key(&source_key).unwrap();
        let target_definition = ObjectDefinitionPlanId(ConeIdentity::SINGLE_FILE.0);
        let patch_key = DigestPatchIntentKey::new(
            source,
            target_definition,
            DefinitionAtomRole::RuntimeRecord,
            DigestSemanticFieldRole::SourceSignature,
        );

        assert_eq!(
            source.to_string(),
            "68ed239bb24f76f50d6c54b3cd54f985e787d8110f841b2009f2613d4025ce1e"
        );
        assert_eq!(
            DigestPatchIntentId::from_key(&patch_key)
                .unwrap()
                .to_string(),
            "3678a782a91d9905b2c16e94b81b33fc29b38614025bb730cf2cc51e7d06e946"
        );
        assert_eq!(patch_key.source(), source);
        assert_eq!(patch_key.target_definition(), target_definition);
        assert_eq!(
            source_key.owner_and_role(),
            DigestOwnerAndRoleKey::SourceSignature(PersistentCallableBodyId(
                ConeIdentity::SINGLE_FILE.0
            ))
        );
    }
}
