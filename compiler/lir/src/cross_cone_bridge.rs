//! Compile-facing LIR bridge for M23-5 ordinary dependency callables.

use scoop_identity::{
    CanonicalScoopAbiFunctionSignature, ConeIdentity, DependencyCallableDeclarationId, GcEffect,
    ObjectDefinitionPlanId, PersistentSymbolRequest, StrongCallableDefinitionOwner,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{CallingConvention, OdrFreeLirFoundation};

mod errors;
mod selection;
mod validation;
mod wire;

pub use errors::*;
pub use selection::*;
pub use wire::DecodedCrossConeLirBridgeSectionV1;

/// Caller-side GC root protocol for one ordinary dependency call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyExternalCallableRootPlanV1 {
    ManagedStatepoint,
    NoGc,
}

impl DependencyExternalCallableRootPlanV1 {
    pub const fn gc_effect(self) -> GcEffect {
        match self {
            Self::ManagedStatepoint => GcEffect::Managed,
            Self::NoGc => GcEffect::NoGc,
        }
    }
}

impl WireEncode for DependencyExternalCallableRootPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::ManagedStatepoint => 1,
            Self::NoGc => 2,
        })
    }
}

impl WireDecode for DependencyExternalCallableRootPlanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::ManagedStatepoint),
            2 => Ok(Self::NoGc),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

/// Canonical LIR contract exported by a terminal provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamFreeLirCallableExportV1 {
    declaration: DependencyCallableDeclarationId,
    target: StrongCallableDefinitionOwner,
    abi_signature: CanonicalScoopAbiFunctionSignature,
    expected_symbol: PersistentSymbolRequest,
    calling_convention: CallingConvention,
    root_plan: DependencyExternalCallableRootPlanV1,
    required_definition: ObjectDefinitionPlanId,
}

impl ParamFreeLirCallableExportV1 {
    pub fn new(
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
        target: StrongCallableDefinitionOwner,
        abi_signature: CanonicalScoopAbiFunctionSignature,
        calling_convention: CallingConvention,
        root_plan: DependencyExternalCallableRootPlanV1,
    ) -> Result<Self, ParamFreeLirCallableBuildError> {
        validation::build_callable(
            provider,
            declaration,
            target,
            abi_signature,
            calling_convention,
            root_plan,
        )
    }

    pub const fn declaration(&self) -> DependencyCallableDeclarationId {
        self.declaration
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

    pub const fn calling_convention(&self) -> CallingConvention {
        self.calling_convention
    }

    pub const fn root_plan(&self) -> DependencyExternalCallableRootPlanV1 {
        self.root_plan
    }

    pub const fn required_definition(&self) -> ObjectDefinitionPlanId {
        self.required_definition
    }
}

impl WireEncode for ParamFreeLirCallableExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.target.encode(encoder)?;
        encoder.field(3)?;
        self.abi_signature.encode(encoder)?;
        encoder.field(4)?;
        self.expected_symbol.encode(encoder)?;
        encoder.field(5)?;
        self.calling_convention.encode(encoder)?;
        encoder.field(6)?;
        self.root_plan.encode(encoder)?;
        encoder.field(7)?;
        self.required_definition.encode(encoder)
    }
}

/// One ordinary dependency callable selected by this consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedDependencyLirCallableV1 {
    provider: ConeIdentity,
    bridge: ParamFreeLirCallableExportV1,
}

impl SelectedDependencyLirCallableV1 {
    pub fn new(
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
        target: StrongCallableDefinitionOwner,
        abi_signature: CanonicalScoopAbiFunctionSignature,
        calling_convention: CallingConvention,
        root_plan: DependencyExternalCallableRootPlanV1,
    ) -> Result<Self, ParamFreeLirCallableBuildError> {
        Ok(Self {
            provider,
            bridge: ParamFreeLirCallableExportV1::new(
                provider,
                declaration,
                target,
                abi_signature,
                calling_convention,
                root_plan,
            )?,
        })
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn bridge(&self) -> &ParamFreeLirCallableExportV1 {
        &self.bridge
    }

    /// Materializes the separately typed LIR external arena entry only after
    /// checking the target-specific physical ABI against this canonical
    /// selected bridge.
    pub fn materialize(
        &self,
        signature: crate::ScoopAbiSignature,
    ) -> Result<crate::DependencyExternalCallable, crate::DependencyExternalBuildError> {
        crate::DependencyExternalCallable::new(self.clone(), signature)
    }

    fn sort_key(&self) -> (ConeIdentity, DependencyCallableDeclarationId) {
        (self.provider, self.bridge.declaration)
    }
}

impl WireEncode for SelectedDependencyLirCallableV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.provider.encode(encoder)?;
        encoder.field(2)?;
        self.bridge.encode(encoder)
    }
}

/// Canonical LIR export and selected-use surfaces for ordinary dependencies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossConeLirBridgeSectionV1 {
    artifact: ConeIdentity,
    exports: Vec<ParamFreeLirCallableExportV1>,
    selected: Vec<SelectedDependencyLirCallableV1>,
}

impl CrossConeLirBridgeSectionV1 {
    pub fn try_new(
        foundation: &OdrFreeLirFoundation,
        mut exports: Vec<ParamFreeLirCallableExportV1>,
        mut selected: Vec<SelectedDependencyLirCallableV1>,
    ) -> Result<Self, CrossConeLirBridgeBuildError> {
        exports.sort_unstable_by_key(ParamFreeLirCallableExportV1::declaration);
        validation::reject_duplicate_exports(&exports)
            .map_err(CrossConeLirBridgeBuildError::DuplicateExport)?;
        selected.sort_unstable_by_key(SelectedDependencyLirCallableV1::sort_key);
        validation::reject_duplicate_selected(&selected).map_err(|(provider, declaration)| {
            CrossConeLirBridgeBuildError::DuplicateSelected {
                provider,
                declaration,
            }
        })?;
        validation::validate_section_relations(foundation, &exports, &selected)
            .map_err(CrossConeLirBridgeBuildError::Relation)?;
        Ok(Self {
            artifact: foundation.producer(),
            exports,
            selected,
        })
    }

    /// Artifact identity supplied by the containing foundation while the
    /// section was built or validated. It is contextual and is not encoded in
    /// this section's wire payload.
    pub const fn artifact(&self) -> ConeIdentity {
        self.artifact
    }

    pub fn exports(&self) -> &[ParamFreeLirCallableExportV1] {
        &self.exports
    }

    pub fn selected(&self) -> &[SelectedDependencyLirCallableV1] {
        &self.selected
    }

    pub fn export(
        &self,
        declaration: DependencyCallableDeclarationId,
    ) -> Option<&ParamFreeLirCallableExportV1> {
        self.exports
            .binary_search_by_key(&declaration, ParamFreeLirCallableExportV1::declaration)
            .ok()
            .map(|index| &self.exports[index])
    }
}

impl WireEncode for CrossConeLirBridgeSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encode_array(encoder, &self.exports)?;
        encoder.field(2)?;
        encode_array(encoder, &self.selected)
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

#[cfg(test)]
mod tests;
