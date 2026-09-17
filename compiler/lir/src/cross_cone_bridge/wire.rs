//! Untrusted wire forms and identity-aware reconstruction.

use scoop_identity::{
    DecodedCanonicalScoopAbiFunctionSignature, DecodedDependencyCallableDeclarationId,
    DecodedPersistentId, DecodedPersistentSymbolRequest, DecodedStrongCallableDefinitionOwner,
    ObjectDefinitionPlanId, PersistentIdResolver, ValidatedIdentityGraph,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, encode};

use crate::{CallingConvention, OdrFreeLirFoundation};

use super::{
    CrossConeLirBridgeSectionV1, CrossConeLirBridgeValidationError,
    DependencyExternalCallableRootPlanV1, ParamFreeLirCallableExportV1,
    ParamFreeLirCallableResolutionError, SelectedDependencyLirCallableResolutionError,
    SelectedDependencyLirCallableV1, validation,
};

#[derive(Debug)]
struct DecodedParamFreeLirCallableExportV1 {
    declaration: DecodedDependencyCallableDeclarationId,
    target: DecodedStrongCallableDefinitionOwner,
    abi_signature: DecodedCanonicalScoopAbiFunctionSignature,
    expected_symbol: DecodedPersistentSymbolRequest,
    calling_convention: CallingConvention,
    root_plan: DependencyExternalCallableRootPlanV1,
    required_definition: DecodedPersistentId<ObjectDefinitionPlanId>,
}

impl DecodedParamFreeLirCallableExportV1 {
    fn reconstruct(
        self,
        provider: scoop_identity::ConeIdentity,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<ParamFreeLirCallableExportV1, ParamFreeLirCallableResolutionError> {
        let declaration = self
            .declaration
            .resolve(identities)
            .map_err(ParamFreeLirCallableResolutionError::Declaration)?;
        let target = self
            .target
            .resolve(identities)
            .map_err(ParamFreeLirCallableResolutionError::Target)?;
        let abi_signature = self
            .abi_signature
            .resolve(identities)
            .map_err(ParamFreeLirCallableResolutionError::Abi)?;
        ParamFreeLirCallableExportV1::new(
            provider,
            declaration,
            target,
            abi_signature,
            self.calling_convention,
            self.root_plan,
        )
        .map_err(ParamFreeLirCallableResolutionError::Shape)
    }
}

impl WireEncode for DecodedParamFreeLirCallableExportV1 {
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

impl WireDecode for DecodedParamFreeLirCallableExportV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(7)?;
        Ok(Self {
            declaration: decoder.field(1, DecodedDependencyCallableDeclarationId::decode)?,
            target: decoder.field(2, DecodedStrongCallableDefinitionOwner::decode)?,
            abi_signature: decoder.field(3, DecodedCanonicalScoopAbiFunctionSignature::decode)?,
            expected_symbol: decoder.field(4, DecodedPersistentSymbolRequest::decode)?,
            calling_convention: decoder.field(5, CallingConvention::decode)?,
            root_plan: decoder.field(6, DependencyExternalCallableRootPlanV1::decode)?,
            required_definition: decoder.field(7, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Debug)]
struct DecodedSelectedDependencyLirCallableV1 {
    provider: DecodedPersistentId<scoop_identity::ConeIdentity>,
    bridge: DecodedParamFreeLirCallableExportV1,
}

impl DecodedSelectedDependencyLirCallableV1 {
    fn reconstruct(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<SelectedDependencyLirCallableV1, SelectedDependencyLirCallableResolutionError> {
        let provider = identities
            .resolve(self.provider)
            .map_err(SelectedDependencyLirCallableResolutionError::Provider)?;
        let bridge = self
            .bridge
            .reconstruct(provider, identities)
            .map_err(SelectedDependencyLirCallableResolutionError::Bridge)?;
        Ok(SelectedDependencyLirCallableV1 { provider, bridge })
    }
}

impl WireEncode for DecodedSelectedDependencyLirCallableV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.provider.encode(encoder)?;
        encoder.field(2)?;
        self.bridge.encode(encoder)
    }
}

impl WireDecode for DecodedSelectedDependencyLirCallableV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            provider: decoder.field(1, DecodedPersistentId::decode)?,
            bridge: decoder.field(2, DecodedParamFreeLirCallableExportV1::decode)?,
        })
    }
}

/// Untrusted wire form of [`CrossConeLirBridgeSectionV1`].
#[derive(Debug)]
pub struct DecodedCrossConeLirBridgeSectionV1 {
    exports: Vec<DecodedParamFreeLirCallableExportV1>,
    selected: Vec<DecodedSelectedDependencyLirCallableV1>,
}

impl DecodedCrossConeLirBridgeSectionV1 {
    pub fn validate(
        self,
        identities: &mut ValidatedIdentityGraph,
        foundation: &OdrFreeLirFoundation,
    ) -> Result<CrossConeLirBridgeSectionV1, CrossConeLirBridgeValidationError> {
        let actual = encode(&self).map_err(CrossConeLirBridgeValidationError::Encode)?;
        let producer = foundation.producer();
        let mut exports = Vec::with_capacity(self.exports.len());
        for (index, decoded) in self.exports.into_iter().enumerate() {
            let record = decoded
                .reconstruct(producer, identities)
                .map_err(|source| CrossConeLirBridgeValidationError::Export { index, source })?;
            validate_export_order(&exports, &record, index)?;
            exports.push(record);
        }

        let mut selected = Vec::with_capacity(self.selected.len());
        for (index, decoded) in self.selected.into_iter().enumerate() {
            let record = decoded
                .reconstruct(identities)
                .map_err(|source| CrossConeLirBridgeValidationError::Selected { index, source })?;
            validate_selected_order(&selected, &record, index)?;
            selected.push(record);
        }

        validation::validate_section_relations(foundation, &exports, &selected)
            .map_err(CrossConeLirBridgeValidationError::Relation)?;
        let expected = CrossConeLirBridgeSectionV1 { exports, selected };
        let expected_bytes =
            encode(&expected).map_err(CrossConeLirBridgeValidationError::Encode)?;
        if actual != expected_bytes {
            return Err(CrossConeLirBridgeValidationError::SectionMismatch);
        }
        Ok(expected)
    }
}

impl WireEncode for DecodedCrossConeLirBridgeSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encode_array(encoder, &self.exports)?;
        encoder.field(2)?;
        encode_array(encoder, &self.selected)
    }
}

impl WireDecode for DecodedCrossConeLirBridgeSectionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            exports: decoder.field(1, |decoder| {
                decoder
                    .decode_array(|decoder, _| DecodedParamFreeLirCallableExportV1::decode(decoder))
            })?,
            selected: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| {
                    DecodedSelectedDependencyLirCallableV1::decode(decoder)
                })
            })?,
        })
    }
}

fn validate_export_order(
    exports: &[ParamFreeLirCallableExportV1],
    record: &ParamFreeLirCallableExportV1,
    index: usize,
) -> Result<(), CrossConeLirBridgeValidationError> {
    let Some(previous) = exports.last() else {
        return Ok(());
    };
    if previous.declaration == record.declaration {
        return Err(CrossConeLirBridgeValidationError::DuplicateExport {
            index,
            declaration: record.declaration,
        });
    }
    if previous.declaration > record.declaration {
        return Err(CrossConeLirBridgeValidationError::NonCanonicalExportOrder { index });
    }
    Ok(())
}

fn validate_selected_order(
    selected: &[SelectedDependencyLirCallableV1],
    record: &SelectedDependencyLirCallableV1,
    index: usize,
) -> Result<(), CrossConeLirBridgeValidationError> {
    let Some(previous) = selected.last() else {
        return Ok(());
    };
    if previous.sort_key() == record.sort_key() {
        return Err(CrossConeLirBridgeValidationError::DuplicateSelected {
            index,
            provider: record.provider,
            declaration: record.bridge.declaration,
        });
    }
    if previous.sort_key() > record.sort_key() {
        return Err(CrossConeLirBridgeValidationError::NonCanonicalSelectedOrder { index });
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
