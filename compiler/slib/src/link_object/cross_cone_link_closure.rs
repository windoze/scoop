//! Link-only semantic and physical closure for ordinary dependency callables.

use std::fmt;

use scoop_identity::{
    CanonicalScoopAbiFunctionSignature, ConeIdentity, ObjectDefinitionPlanId,
    PersistentSymbolRequest, StrongCallableDefinitionOwner,
};
use scoop_lir::{CrossConeLirBridgeSectionV1, SelectedDependencyLirCallableV1};
use scoop_wire::{Encoder, HashError, WireEncode, domain_separated_cbor_hash};

use super::{
    CanonicalUndefinedRelocationUseV1, CodeLinkObjectMemberSetV1, VerifiedCodeLinkObjectMemberSetV1,
};
use crate::SlibMemberId;

const OBJECT_COVERAGE_DOMAIN: &str = "scoop-cross-cone-object-coverage-v1";

mod classification;
pub use classification::*;

mod wire;
pub use wire::{CrossConeLinkClosureSectionValidationError, DecodedCrossConeLinkClosureSectionV1};

#[cfg(test)]
mod tests;

/// Code-fingerprint projection of one selected ordinary dependency callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossConeLinkSemanticImportV1 {
    provider: ConeIdentity,
    target: StrongCallableDefinitionOwner,
    abi_signature: CanonicalScoopAbiFunctionSignature,
    expected_symbol: PersistentSymbolRequest,
    required_definition: ObjectDefinitionPlanId,
}

impl CrossConeLinkSemanticImportV1 {
    fn from_selected(selected: &SelectedDependencyLirCallableV1) -> Self {
        Self {
            provider: selected.provider(),
            target: selected.bridge().target(),
            abi_signature: selected.bridge().abi_signature().clone(),
            expected_symbol: selected.bridge().expected_symbol(),
            required_definition: selected.bridge().required_definition(),
        }
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn target(&self) -> StrongCallableDefinitionOwner {
        self.target
    }

    pub const fn abi_signature(&self) -> &CanonicalScoopAbiFunctionSignature {
        &self.abi_signature
    }

    pub const fn expected_symbol(&self) -> PersistentSymbolRequest {
        self.expected_symbol
    }

    pub const fn required_definition(&self) -> ObjectDefinitionPlanId {
        self.required_definition
    }

    const fn sort_key(&self) -> (ConeIdentity, StrongCallableDefinitionOwner) {
        (self.provider, self.target)
    }
}

impl WireEncode for CrossConeLinkSemanticImportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.provider.encode(encoder)?;
        encoder.field(2)?;
        self.target.encode(encoder)?;
        encoder.field(3)?;
        self.abi_signature.encode(encoder)?;
        encoder.field(4)?;
        self.expected_symbol.encode(encoder)?;
        encoder.field(5)?;
        self.required_definition.encode(encoder)
    }
}

/// Canonical Code contribution projected from one consumer's LIR selections.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossConeLinkSemanticImportSetV1 {
    consumer: ConeIdentity,
    imports: Vec<CrossConeLinkSemanticImportV1>,
}

impl CrossConeLinkSemanticImportSetV1 {
    pub fn from_lir_bridge(
        bridge: &CrossConeLirBridgeSectionV1,
    ) -> Result<Self, CrossConeLinkSemanticImportBuildError> {
        let mut imports = bridge
            .selected()
            .iter()
            .map(CrossConeLinkSemanticImportV1::from_selected)
            .collect::<Vec<_>>();
        imports.sort_unstable_by_key(CrossConeLinkSemanticImportV1::sort_key);
        if let Some(pair) = imports
            .windows(2)
            .find(|pair| pair[0].sort_key() == pair[1].sort_key())
        {
            return Err(CrossConeLinkSemanticImportBuildError::DuplicateImport {
                provider: pair[0].provider,
                target: pair[0].target,
            });
        }
        if imports.len() > u32::MAX as usize {
            return Err(CrossConeLinkSemanticImportBuildError::TooManyImports {
                actual: imports.len(),
            });
        }
        Ok(Self {
            consumer: bridge.artifact(),
            imports,
        })
    }

    pub const fn consumer(&self) -> ConeIdentity {
        self.consumer
    }

    pub fn imports(&self) -> &[CrossConeLinkSemanticImportV1] {
        &self.imports
    }
}

impl WireEncode for CrossConeLinkSemanticImportSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.imports)
    }
}

/// One verified physical relocation assigned to a semantic import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossConeUndefinedRequirementV1 {
    use_site: CanonicalUndefinedRelocationUseV1,
    import_index: u32,
}

impl CrossConeUndefinedRequirementV1 {
    pub const fn use_site(&self) -> &CanonicalUndefinedRelocationUseV1 {
        &self.use_site
    }

    pub const fn import_index(&self) -> u32 {
        self.import_index
    }
}

impl WireEncode for CrossConeUndefinedRequirementV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.use_site.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.import_index))
    }
}

/// Digest binding the complete final LinkObject set to all cross-Cone uses.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CrossConeRelocationUseSetDigestV1([u8; 32]);

impl CrossConeRelocationUseSetDigestV1 {
    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for CrossConeRelocationUseSetDigestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for CrossConeRelocationUseSetDigestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// LinkValidationOnly proof that the physical use set covers final objects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossConeObjectCoverageProofV1 {
    verified_link_objects: CodeLinkObjectMemberSetV1,
    relocation_use_set_digest: CrossConeRelocationUseSetDigestV1,
}

impl CrossConeObjectCoverageProofV1 {
    pub const fn verified_link_objects(&self) -> &CodeLinkObjectMemberSetV1 {
        &self.verified_link_objects
    }

    pub const fn relocation_use_set_digest(&self) -> CrossConeRelocationUseSetDigestV1 {
        self.relocation_use_set_digest
    }
}

impl WireEncode for CrossConeObjectCoverageProofV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.verified_link_objects.encode(encoder)?;
        encoder.field(2)?;
        self.relocation_use_set_digest.encode(encoder)
    }
}

/// Canonical payload of `org.scoop-lang.lir/cross-cone-link-closure/1`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossConeLinkClosureSectionV1 {
    semantic_imports: CrossConeLinkSemanticImportSetV1,
    requirements: Vec<CrossConeUndefinedRequirementV1>,
    object_coverage: CrossConeObjectCoverageProofV1,
}

impl CrossConeLinkClosureSectionV1 {
    pub fn from_verified_requirements(
        requirements: &VerifiedCrossConeStrongRequirementClosureV1,
        link_objects: &VerifiedCodeLinkObjectMemberSetV1,
    ) -> Result<Self, CrossConeLinkClosureBuildError> {
        if requirements.producer() != link_objects.producer() {
            return Err(CrossConeLinkClosureBuildError::ProducerMismatch {
                requirements: requirements.producer(),
                objects: link_objects.producer(),
            });
        }
        Self::from_parts(
            requirements.semantic_imports().clone(),
            requirements.requirements().to_vec(),
            link_objects.projection().clone(),
        )
    }

    fn from_parts(
        semantic_imports: CrossConeLinkSemanticImportSetV1,
        requirements: Vec<CrossConeUndefinedRequirementV1>,
        verified_link_objects: CodeLinkObjectMemberSetV1,
    ) -> Result<Self, CrossConeLinkClosureBuildError> {
        for requirement in &requirements {
            let member = requirement.use_site.source_member();
            if verified_link_objects
                .members()
                .binary_search_by_key(&member, |entry| entry.member())
                .is_err()
            {
                return Err(CrossConeLinkClosureBuildError::UseOutsideObjectSet { member });
            }
        }
        let digest = object_coverage_digest(&verified_link_objects, &requirements)
            .map_err(CrossConeLinkClosureBuildError::Hash)?;
        Ok(Self {
            semantic_imports,
            requirements,
            object_coverage: CrossConeObjectCoverageProofV1 {
                verified_link_objects,
                relocation_use_set_digest: digest,
            },
        })
    }

    pub const fn consumer(&self) -> ConeIdentity {
        self.semantic_imports.consumer()
    }

    pub const fn semantic_imports(&self) -> &CrossConeLinkSemanticImportSetV1 {
        &self.semantic_imports
    }

    pub fn requirements(&self) -> &[CrossConeUndefinedRequirementV1] {
        &self.requirements
    }

    pub const fn object_coverage(&self) -> &CrossConeObjectCoverageProofV1 {
        &self.object_coverage
    }
}

impl WireEncode for CrossConeLinkClosureSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.semantic_imports.encode(encoder)?;
        encoder.field(2)?;
        encode_array(encoder, &self.requirements)?;
        encoder.field(3)?;
        self.object_coverage.encode(encoder)
    }
}

struct CrossConeObjectCoveragePreimageV1<'a> {
    verified_link_objects: &'a CodeLinkObjectMemberSetV1,
    requirements: &'a [CrossConeUndefinedRequirementV1],
}

impl WireEncode for CrossConeObjectCoveragePreimageV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.verified_link_objects.encode(encoder)?;
        encoder.field(2)?;
        encoder.array(self.requirements.len() as u64)?;
        for requirement in self.requirements {
            requirement.use_site.encode(encoder)?;
        }
        Ok(())
    }
}

fn object_coverage_digest(
    verified_link_objects: &CodeLinkObjectMemberSetV1,
    requirements: &[CrossConeUndefinedRequirementV1],
) -> Result<CrossConeRelocationUseSetDigestV1, HashError> {
    domain_separated_cbor_hash(
        OBJECT_COVERAGE_DOMAIN,
        &CrossConeObjectCoveragePreimageV1 {
            verified_link_objects,
            requirements,
        },
    )
    .map(|digest| CrossConeRelocationUseSetDigestV1(*digest.as_array()))
}

fn encode_array<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

pub(in crate::link_object) type UseKey = (
    SlibMemberId,
    scoop_identity::ObjectDefinitionAtomId,
    u64,
    super::RelocationTargetSlotV1,
);

pub(in crate::link_object) fn use_key(use_site: &CanonicalUndefinedRelocationUseV1) -> UseKey {
    (
        use_site.source_member(),
        use_site.containing_atom(),
        use_site.offset_within_atom(),
        use_site.target_slot(),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrossConeLinkSemanticImportBuildError {
    DuplicateImport {
        provider: ConeIdentity,
        target: StrongCallableDefinitionOwner,
    },
    TooManyImports {
        actual: usize,
    },
}

impl fmt::Display for CrossConeLinkSemanticImportBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid cross-Cone link semantic imports: {self:?}"
        )
    }
}

impl std::error::Error for CrossConeLinkSemanticImportBuildError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConeLinkClosureBuildError {
    ProducerMismatch {
        requirements: ConeIdentity,
        objects: ConeIdentity,
    },
    UseOutsideObjectSet {
        member: SlibMemberId,
    },
    Hash(HashError),
}

impl fmt::Display for CrossConeLinkClosureBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot build cross-Cone link closure: {self:?}")
    }
}

impl std::error::Error for CrossConeLinkClosureBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hash(source) => Some(source),
            Self::ProducerMismatch { .. } | Self::UseOutsideObjectSet { .. } => None,
        }
    }
}
