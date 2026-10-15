//! Ordinary dependency imports and actual undefined symbol uses.

use std::fmt;

use scoop_identity::{
    CanonicalScoopAbiFunctionSignature, ConeIdentity, ObjectDefinitionPlanId,
    PersistentSymbolRequest, StrongCallableDefinitionOwner,
};
use scoop_lir::{CrossConeLirBridgeSectionV1, SelectedDependencyLirCallableV1};
use scoop_wire::{Encoder, WireEncode};

use super::CanonicalUndefinedRelocationUseV1;
use crate::SlibMemberId;

mod classification;
pub use classification::*;

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
