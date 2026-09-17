use scoop_identity::{
    ConeIdentity, DependencyCallableDeclarationId, ExactCallableSignature,
    StrongCallableDefinitionOwner,
};
use scoop_wire::{Encoder, WireEncode};

use crate::OdrFreeMirFoundation;

use super::encode_array;
use super::errors::{CrossConeMirBridgeBuildError, ParamFreeMirCallableBuildError};
use super::validation::{
    reject_duplicate_exports, reject_duplicate_selected, validate_callable_shape,
    validate_section_relations,
};

/// One executable callable exported by its terminal provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamFreeMirCallableExportV1 {
    pub(super) declaration: DependencyCallableDeclarationId,
    pub(super) implementation: StrongCallableDefinitionOwner,
    pub(super) signature: ExactCallableSignature,
}

impl ParamFreeMirCallableExportV1 {
    pub fn try_new(
        declaration: DependencyCallableDeclarationId,
        implementation: StrongCallableDefinitionOwner,
        signature: ExactCallableSignature,
    ) -> Result<Self, ParamFreeMirCallableBuildError> {
        validate_callable_shape(declaration, implementation, &signature)?;
        Ok(Self {
            declaration,
            implementation,
            signature,
        })
    }

    pub const fn declaration(&self) -> DependencyCallableDeclarationId {
        self.declaration
    }

    pub const fn implementation(&self) -> StrongCallableDefinitionOwner {
        self.implementation
    }

    pub const fn signature(&self) -> &ExactCallableSignature {
        &self.signature
    }
}

impl WireEncode for ParamFreeMirCallableExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.implementation.encode(encoder)?;
        encoder.field(3)?;
        self.signature.encode(encoder)
    }
}

/// One ordinary dependency callable selected by this consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedDependencyMirCallableV1 {
    pub(super) provider: ConeIdentity,
    pub(super) declaration: DependencyCallableDeclarationId,
    pub(super) implementation: StrongCallableDefinitionOwner,
    pub(super) signature: ExactCallableSignature,
}

impl SelectedDependencyMirCallableV1 {
    pub fn try_new(
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
        implementation: StrongCallableDefinitionOwner,
        signature: ExactCallableSignature,
    ) -> Result<Self, ParamFreeMirCallableBuildError> {
        validate_callable_shape(declaration, implementation, &signature)?;
        Ok(Self {
            provider,
            declaration,
            implementation,
            signature,
        })
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn declaration(&self) -> DependencyCallableDeclarationId {
        self.declaration
    }

    pub const fn implementation(&self) -> StrongCallableDefinitionOwner {
        self.implementation
    }

    pub const fn signature(&self) -> &ExactCallableSignature {
        &self.signature
    }

    pub(super) fn sort_key(&self) -> (ConeIdentity, DependencyCallableDeclarationId) {
        (self.provider, self.declaration)
    }
}

impl WireEncode for SelectedDependencyMirCallableV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.provider.encode(encoder)?;
        encoder.field(2)?;
        self.declaration.encode(encoder)?;
        encoder.field(3)?;
        self.implementation.encode(encoder)?;
        encoder.field(4)?;
        self.signature.encode(encoder)
    }
}

/// Canonical MIR export and selected-use surfaces for ordinary dependencies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossConeMirBridgeSectionV1 {
    pub(super) exports: Vec<ParamFreeMirCallableExportV1>,
    pub(super) selected: Vec<SelectedDependencyMirCallableV1>,
}

impl CrossConeMirBridgeSectionV1 {
    pub fn try_new(
        artifact: ConeIdentity,
        foundation: &OdrFreeMirFoundation,
        mut exports: Vec<ParamFreeMirCallableExportV1>,
        mut selected: Vec<SelectedDependencyMirCallableV1>,
    ) -> Result<Self, CrossConeMirBridgeBuildError> {
        exports.sort_unstable_by_key(ParamFreeMirCallableExportV1::declaration);
        reject_duplicate_exports(&exports)
            .map_err(CrossConeMirBridgeBuildError::DuplicateExport)?;
        selected.sort_unstable_by_key(SelectedDependencyMirCallableV1::sort_key);
        reject_duplicate_selected(&selected).map_err(|(provider, declaration)| {
            CrossConeMirBridgeBuildError::DuplicateSelected {
                provider,
                declaration,
            }
        })?;
        validate_section_relations(artifact, foundation, &exports, &selected)
            .map_err(CrossConeMirBridgeBuildError::Relation)?;
        Ok(Self { exports, selected })
    }

    pub fn exports(&self) -> &[ParamFreeMirCallableExportV1] {
        &self.exports
    }

    pub fn selected(&self) -> &[SelectedDependencyMirCallableV1] {
        &self.selected
    }
}

impl WireEncode for CrossConeMirBridgeSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encode_array(encoder, &self.exports)?;
        encoder.field(2)?;
        encode_array(encoder, &self.selected)
    }
}
