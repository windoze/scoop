//! Compile-facing MIR bridge for M23-5 ordinary dependency callables.

use scoop_identity::{
    ConeIdentity, DecodedDependencyCallableDeclarationId, DecodedExactCallableSignature,
    DecodedPersistentId, DecodedStrongCallableDefinitionOwner, DependencyCallableDeclarationId,
    ExactCallableSignature, PersistentIdResolver, StrongCallableDefinitionOwner,
    ValidatedIdentityGraph,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::{CallableSignatureSubject, OdrFreeMirFoundation};

mod errors;

pub use errors::*;

/// One executable callable exported by its terminal provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamFreeMirCallableExportV1 {
    declaration: DependencyCallableDeclarationId,
    implementation: StrongCallableDefinitionOwner,
    signature: ExactCallableSignature,
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
    provider: ConeIdentity,
    declaration: DependencyCallableDeclarationId,
    implementation: StrongCallableDefinitionOwner,
    signature: ExactCallableSignature,
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

    fn sort_key(&self) -> (ConeIdentity, DependencyCallableDeclarationId) {
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
    exports: Vec<ParamFreeMirCallableExportV1>,
    selected: Vec<SelectedDependencyMirCallableV1>,
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

#[derive(Debug)]
struct DecodedParamFreeMirCallableExportV1 {
    declaration: DecodedDependencyCallableDeclarationId,
    implementation: DecodedStrongCallableDefinitionOwner,
    signature: DecodedExactCallableSignature,
}

impl DecodedParamFreeMirCallableExportV1 {
    fn resolve(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<ParamFreeMirCallableExportV1, ParamFreeMirCallableResolutionError> {
        let declaration = self
            .declaration
            .resolve(identities)
            .map_err(ParamFreeMirCallableResolutionError::Declaration)?;
        let implementation = self
            .implementation
            .resolve(identities)
            .map_err(ParamFreeMirCallableResolutionError::Implementation)?;
        let signature = self
            .signature
            .resolve(identities)
            .map_err(ParamFreeMirCallableResolutionError::Signature)?;
        ParamFreeMirCallableExportV1::try_new(declaration, implementation, signature)
            .map_err(ParamFreeMirCallableResolutionError::Shape)
    }
}

impl WireEncode for DecodedParamFreeMirCallableExportV1 {
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

impl WireDecode for DecodedParamFreeMirCallableExportV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            declaration: decoder.field(1, DecodedDependencyCallableDeclarationId::decode)?,
            implementation: decoder.field(2, DecodedStrongCallableDefinitionOwner::decode)?,
            signature: decoder.field(3, DecodedExactCallableSignature::decode)?,
        })
    }
}

#[derive(Debug)]
struct DecodedSelectedDependencyMirCallableV1 {
    provider: DecodedPersistentId<ConeIdentity>,
    declaration: DecodedDependencyCallableDeclarationId,
    implementation: DecodedStrongCallableDefinitionOwner,
    signature: DecodedExactCallableSignature,
}

impl DecodedSelectedDependencyMirCallableV1 {
    fn resolve(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<SelectedDependencyMirCallableV1, SelectedDependencyMirCallableResolutionError> {
        let provider = identities
            .resolve(self.provider)
            .map_err(SelectedDependencyMirCallableResolutionError::Provider)?;
        let declaration = self
            .declaration
            .resolve(identities)
            .map_err(SelectedDependencyMirCallableResolutionError::Declaration)?;
        let implementation = self
            .implementation
            .resolve(identities)
            .map_err(SelectedDependencyMirCallableResolutionError::Implementation)?;
        let signature = self
            .signature
            .resolve(identities)
            .map_err(SelectedDependencyMirCallableResolutionError::Signature)?;
        SelectedDependencyMirCallableV1::try_new(provider, declaration, implementation, signature)
            .map_err(SelectedDependencyMirCallableResolutionError::Shape)
    }
}

impl WireEncode for DecodedSelectedDependencyMirCallableV1 {
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

/// Untrusted wire form of [`CrossConeMirBridgeSectionV1`].
#[derive(Debug)]
pub struct DecodedCrossConeMirBridgeSectionV1 {
    exports: Vec<DecodedParamFreeMirCallableExportV1>,
    selected: Vec<DecodedSelectedDependencyMirCallableV1>,
}

impl DecodedCrossConeMirBridgeSectionV1 {
    pub fn validate(
        self,
        artifact: ConeIdentity,
        identities: &mut ValidatedIdentityGraph,
        foundation: &OdrFreeMirFoundation,
    ) -> Result<CrossConeMirBridgeSectionV1, CrossConeMirBridgeValidationError> {
        let mut exports = Vec::with_capacity(self.exports.len());
        for (index, decoded) in self.exports.into_iter().enumerate() {
            let record = decoded
                .resolve(identities)
                .map_err(|source| CrossConeMirBridgeValidationError::Export { index, source })?;
            validate_export_order(&exports, &record, index)?;
            exports.push(record);
        }

        let mut selected = Vec::with_capacity(self.selected.len());
        for (index, decoded) in self.selected.into_iter().enumerate() {
            let record = decoded
                .resolve(identities)
                .map_err(|source| CrossConeMirBridgeValidationError::Selected { index, source })?;
            validate_selected_order(&selected, &record, index)?;
            selected.push(record);
        }

        validate_section_relations(artifact, foundation, &exports, &selected)
            .map_err(CrossConeMirBridgeValidationError::Relation)?;
        Ok(CrossConeMirBridgeSectionV1 { exports, selected })
    }
}

impl WireEncode for DecodedCrossConeMirBridgeSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encode_array(encoder, &self.exports)?;
        encoder.field(2)?;
        encoder.array(self.selected.len() as u64)?;
        for record in &self.selected {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCrossConeMirBridgeSectionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            exports: decoder.field(1, |decoder| {
                decoder
                    .decode_array(|decoder, _| DecodedParamFreeMirCallableExportV1::decode(decoder))
            })?,
            selected: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| {
                    DecodedSelectedDependencyMirCallableV1::decode(decoder)
                })
            })?,
        })
    }
}

impl WireDecode for DecodedSelectedDependencyMirCallableV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            provider: decoder.field(1, DecodedPersistentId::decode)?,
            declaration: decoder.field(2, DecodedDependencyCallableDeclarationId::decode)?,
            implementation: decoder.field(3, DecodedStrongCallableDefinitionOwner::decode)?,
            signature: decoder.field(4, DecodedExactCallableSignature::decode)?,
        })
    }
}

fn validate_callable_shape(
    declaration: DependencyCallableDeclarationId,
    implementation: StrongCallableDefinitionOwner,
    signature: &ExactCallableSignature,
) -> Result<(), ParamFreeMirCallableBuildError> {
    let expected = declaration.implementation();
    if implementation != expected {
        return Err(ParamFreeMirCallableBuildError::ImplementationMismatch {
            declaration,
            expected,
            actual: implementation,
        });
    }
    if signature.effect() != scoop_identity::Effect::Ordinary {
        return Err(ParamFreeMirCallableBuildError::Suspend { declaration });
    }
    Ok(())
}

fn validate_section_relations(
    artifact: ConeIdentity,
    foundation: &OdrFreeMirFoundation,
    exports: &[ParamFreeMirCallableExportV1],
    selected: &[SelectedDependencyMirCallableV1],
) -> Result<(), CrossConeMirBridgeRelationError> {
    if artifact == ConeIdentity::CORE && !exports.is_empty() {
        return Err(CrossConeMirBridgeRelationError::CoreExportsOrdinaryDependencyCallable);
    }
    for (index, export) in exports.iter().enumerate() {
        let subject = CallableSignatureSubject::Strong(export.implementation.callable_owner());
        let Some(expected) = foundation
            .as_canonical()
            .callable_signatures()
            .iter()
            .find(|record| record.subject() == subject)
        else {
            return Err(
                CrossConeMirBridgeRelationError::MissingExportImplementation {
                    index,
                    implementation: export.implementation,
                },
            );
        };
        if expected.signature() != &export.signature {
            return Err(CrossConeMirBridgeRelationError::ExportSignatureMismatch {
                index,
                implementation: export.implementation,
            });
        }
    }
    for (index, selected) in selected.iter().enumerate() {
        if selected.provider == artifact {
            return Err(CrossConeMirBridgeRelationError::SelectedCurrentProvider {
                index,
                provider: selected.provider,
            });
        }
        if selected.provider == ConeIdentity::CORE {
            return Err(CrossConeMirBridgeRelationError::SelectedTrustedCore { index });
        }
    }
    Ok(())
}

fn reject_duplicate_exports(
    exports: &[ParamFreeMirCallableExportV1],
) -> Result<(), DependencyCallableDeclarationId> {
    exports
        .windows(2)
        .find(|pair| pair[0].declaration == pair[1].declaration)
        .map_or(Ok(()), |pair| Err(pair[0].declaration))
}

fn reject_duplicate_selected(
    selected: &[SelectedDependencyMirCallableV1],
) -> Result<(), (ConeIdentity, DependencyCallableDeclarationId)> {
    selected
        .windows(2)
        .find(|pair| pair[0].sort_key() == pair[1].sort_key())
        .map_or(Ok(()), |pair| Err(pair[0].sort_key()))
}

fn validate_export_order(
    exports: &[ParamFreeMirCallableExportV1],
    record: &ParamFreeMirCallableExportV1,
    index: usize,
) -> Result<(), CrossConeMirBridgeValidationError> {
    let Some(previous) = exports.last() else {
        return Ok(());
    };
    if previous.declaration == record.declaration {
        return Err(CrossConeMirBridgeValidationError::DuplicateExport {
            index,
            declaration: record.declaration,
        });
    }
    if previous.declaration > record.declaration {
        return Err(CrossConeMirBridgeValidationError::NonCanonicalExportOrder { index });
    }
    Ok(())
}

fn validate_selected_order(
    selected: &[SelectedDependencyMirCallableV1],
    record: &SelectedDependencyMirCallableV1,
    index: usize,
) -> Result<(), CrossConeMirBridgeValidationError> {
    let Some(previous) = selected.last() else {
        return Ok(());
    };
    if previous.sort_key() == record.sort_key() {
        return Err(CrossConeMirBridgeValidationError::DuplicateSelected {
            index,
            provider: record.provider,
            declaration: record.declaration,
        });
    }
    if previous.sort_key() > record.sort_key() {
        return Err(CrossConeMirBridgeValidationError::NonCanonicalSelectedOrder { index });
    }
    Ok(())
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
