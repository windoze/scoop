//! Compile-facing LIR bridge for M23-5 ordinary dependency callables.

use scoop_identity::{
    CanonicalScoopAbiFunctionSignature, ConeIdentity, DependencyCallableDeclarationId,
    ObjectDefinitionPlanId, PersistentSymbolRequest, StrongCallableDefinitionOwner,
};
use scoop_wire::{Encoder, WireEncode};

use crate::{
    CallableAbiRecordV1, CallingConvention, ExternalCallableRootPlan, OdrFreeLirFoundation,
};

mod errors;
mod selection;
mod validation;
mod wire;

pub use errors::*;
pub use selection::*;
pub use wire::DecodedCrossConeLirBridgeSectionV1;
pub(crate) use wire::DecodedSelectedDependencyLirCallableV1;

/// Canonical LIR contract exported by a terminal provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamFreeLirCallableExportV1 {
    declaration: DependencyCallableDeclarationId,
    callable: CallableAbiRecordV1,
}

impl ParamFreeLirCallableExportV1 {
    pub fn new(
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
        target: StrongCallableDefinitionOwner,
        abi_signature: CanonicalScoopAbiFunctionSignature,
        calling_convention: CallingConvention,
        root_plan: ExternalCallableRootPlan,
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

    /// Attaches a declared callable to its complete, already constructed ABI.
    /// The declaration must identify exactly this implementation.
    pub fn from_abi(
        declaration: DependencyCallableDeclarationId,
        callable: CallableAbiRecordV1,
    ) -> Result<Self, ParamFreeLirCallableBuildError> {
        let expected = declaration.implementation();
        if callable.target() != expected {
            return Err(ParamFreeLirCallableBuildError::TargetMismatch {
                declaration,
                expected,
                actual: callable.target(),
            });
        }
        Ok(Self {
            declaration,
            callable,
        })
    }

    pub const fn callable_abi(&self) -> &CallableAbiRecordV1 {
        &self.callable
    }

    pub const fn declaration(&self) -> DependencyCallableDeclarationId {
        self.declaration
    }

    pub const fn target(&self) -> StrongCallableDefinitionOwner {
        self.callable.target()
    }

    pub const fn abi_signature(&self) -> &CanonicalScoopAbiFunctionSignature {
        self.callable.abi_signature()
    }

    pub const fn expected_symbol(&self) -> PersistentSymbolRequest {
        self.callable.expected_symbol()
    }

    pub const fn calling_convention(&self) -> CallingConvention {
        self.callable.calling_convention()
    }

    pub const fn root_plan(&self) -> ExternalCallableRootPlan {
        self.callable.root_plan()
    }

    pub const fn required_definition(&self) -> ObjectDefinitionPlanId {
        self.callable.required_definition()
    }
}

impl WireEncode for ParamFreeLirCallableExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.callable.encode(encoder)
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
        root_plan: ExternalCallableRootPlan,
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
    ) -> Result<crate::ExternalCallable, crate::ExternalCallableBuildError> {
        crate::ExternalCallable::new(self.clone(), crate::CallableRole::Ordinary, signature)
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
        validation::canonical_order(&mut exports, &mut selected)?;
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
