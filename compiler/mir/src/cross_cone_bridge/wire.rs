use scoop_identity::{
    ConeIdentity, DecodedDependencyCallableDeclarationId, DecodedExactCallableSignature,
    DecodedPersistentId, DecodedStrongCallableDefinitionOwner, PersistentIdResolver,
    ValidatedIdentityGraph,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

use crate::OdrFreeMirFoundation;

use super::encode_array;
use super::errors::{
    CrossConeMirBridgeValidationError, ParamFreeMirCallableResolutionError,
    SelectedDependencyMirCallableResolutionError,
};
use super::model::{
    CrossConeMirBridgeSectionV1, ParamFreeMirCallableExportV1, SelectedDependencyMirCallableV1,
};
use super::validation::{
    validate_export_order, validate_section_relations, validate_selected_order,
};

#[derive(Debug)]
pub(super) struct DecodedParamFreeMirCallableExportV1 {
    declaration: DecodedDependencyCallableDeclarationId,
    implementation: DecodedStrongCallableDefinitionOwner,
    signature: DecodedExactCallableSignature,
    gc_effect: crate::GcEffect,
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
        ParamFreeMirCallableExportV1::try_new(
            declaration,
            implementation,
            signature,
            self.gc_effect,
        )
        .map_err(ParamFreeMirCallableResolutionError::Shape)
    }
}

impl WireEncode for DecodedParamFreeMirCallableExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.implementation.encode(encoder)?;
        encoder.field(3)?;
        self.signature.encode(encoder)?;
        encoder.field(4)?;
        encoder.unsigned(match self.gc_effect {
            crate::GcEffect::Managed => 1,
            crate::GcEffect::NoGc => 2,
        })
    }
}

impl WireDecode for DecodedParamFreeMirCallableExportV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            declaration: decoder.field(1, DecodedDependencyCallableDeclarationId::decode)?,
            implementation: decoder.field(2, DecodedStrongCallableDefinitionOwner::decode)?,
            signature: decoder.field(3, DecodedExactCallableSignature::decode)?,
            gc_effect: decoder.field(4, |decoder| match decoder.unsigned()? {
                1 => Ok(crate::GcEffect::Managed),
                2 => Ok(crate::GcEffect::NoGc),
                tag => Err(WireError::new(
                    scoop_wire::WireErrorKind::UnknownTag { tag },
                    decoder.path().clone(),
                    Some(decoder.position()),
                )),
            })?,
        })
    }
}

#[derive(Debug)]
pub(super) struct DecodedSelectedDependencyMirCallableV1 {
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
    pub(super) exports: Vec<DecodedParamFreeMirCallableExportV1>,
    pub(super) selected: Vec<DecodedSelectedDependencyMirCallableV1>,
}

impl DecodedCrossConeMirBridgeSectionV1 {
    pub fn validate(
        self,
        artifact: ConeIdentity,
        identities: &mut ValidatedIdentityGraph,
        foundation: &OdrFreeMirFoundation,
    ) -> Result<CrossConeMirBridgeSectionV1, CrossConeMirBridgeValidationError> {
        let mut exports = reserve_table(self.exports.len(), 1)?;
        for (index, decoded) in self.exports.into_iter().enumerate() {
            let record = decoded
                .resolve(identities)
                .map_err(|source| CrossConeMirBridgeValidationError::Export { index, source })?;
            validate_export_order(&exports, &record, index)?;
            exports.push(record);
        }

        let mut selected = reserve_table(self.selected.len(), 2)?;
        for (index, decoded) in self.selected.into_iter().enumerate() {
            let record = decoded
                .resolve(identities)
                .map_err(|source| CrossConeMirBridgeValidationError::Selected { index, source })?;
            validate_selected_order(&selected, &record, index)?;
            selected.push(record);
        }

        validate_section_relations(artifact, foundation, &exports, &selected)
            .map_err(CrossConeMirBridgeValidationError::Relation)?;
        Ok(CrossConeMirBridgeSectionV1 {
            artifact,
            exports: exports.into(),
            selected: selected.into(),
        })
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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            provider: decoder.field(1, DecodedPersistentId::decode)?,
            declaration: decoder.field(2, DecodedDependencyCallableDeclarationId::decode)?,
            implementation: decoder.field(3, DecodedStrongCallableDefinitionOwner::decode)?,
            signature: decoder.field(4, DecodedExactCallableSignature::decode)?,
        })
    }
}

fn reserve_table<T>(
    length: usize,

    field: u32,
) -> Result<Vec<T>, CrossConeMirBridgeValidationError> {
    let path = WirePath::root().field(field);

    let mut result = Vec::new();
    scoop_wire::allocation::try_reserve(&mut result, length, &path)
        .map_err(CrossConeMirBridgeValidationError::Resource)?;
    Ok(result)
}
