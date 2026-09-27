use scoop_identity::{
    DecodedExactCallableSignature, DecodedPersistentId, DecodedPersistentSymbolRequest,
    DigestPatchIntentId, ExactOrdinaryNoArgUnitSignature, ExecutableSourceEntryIdentity,
    MainCallableBodyId, ObjectDefinitionPlanId, PersistentCallableBodyId, PersistentFunctionId,
    PersistentStaticStorageId, PersistentSymbolError, PersistentSymbolKey, PersistentSymbolRequest,
    SourceSignatureFingerprint,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, encode};

use crate::{
    ConeLirFoundation, StrongDigestFinalizationPlanV1, StrongRegistrationIdentitySurfaceV1,
};

mod validation;
pub use validation::{EntryProductionPlanBuildError, EntryProductionPlanValidationError};
use validation::{build_executable, validate_library_foundation};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntryProductionSourceV1 {
    Library,
    Executable(Box<ExecutableSourceEntryIdentity>),
}

impl EntryProductionSourceV1 {
    pub fn executable(entry: ExecutableSourceEntryIdentity) -> Self {
        Self::Executable(Box::new(entry))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableEntryPlanV1 {
    root_cone: scoop_identity::ConeIdentity,
    declaration: PersistentFunctionId,
    source_signature: ExactOrdinaryNoArgUnitSignature,
    source_signature_fingerprint: SourceSignatureFingerprint,
    main: MainCallableBodyId,
    failure_root: PersistentStaticStorageId,
    gateway: PersistentCallableBodyId,
    root_descriptor_symbol: PersistentSymbolRequest,
    root_descriptor_definition: ObjectDefinitionPlanId,
    source_signature_patch: DigestPatchIntentId,
    gateway_definition_patch: DigestPatchIntentId,
}

impl ExecutableEntryPlanV1 {
    pub const fn root_cone(&self) -> scoop_identity::ConeIdentity {
        self.root_cone
    }

    pub const fn declaration(&self) -> PersistentFunctionId {
        self.declaration
    }

    pub const fn source_signature(&self) -> &ExactOrdinaryNoArgUnitSignature {
        &self.source_signature
    }

    pub const fn source_signature_fingerprint(&self) -> SourceSignatureFingerprint {
        self.source_signature_fingerprint
    }

    pub const fn main(&self) -> MainCallableBodyId {
        self.main
    }

    pub const fn failure_root(&self) -> PersistentStaticStorageId {
        self.failure_root
    }

    pub const fn gateway(&self) -> PersistentCallableBodyId {
        self.gateway
    }

    pub const fn root_descriptor_symbol(&self) -> PersistentSymbolRequest {
        self.root_descriptor_symbol
    }

    pub const fn root_descriptor_definition(&self) -> ObjectDefinitionPlanId {
        self.root_descriptor_definition
    }

    pub const fn source_signature_patch(&self) -> DigestPatchIntentId {
        self.source_signature_patch
    }

    pub const fn gateway_definition_patch(&self) -> DigestPatchIntentId {
        self.gateway_definition_patch
    }

    pub fn failure_root_registration_symbol(
        &self,
    ) -> Result<PersistentSymbolRequest, PersistentSymbolError> {
        PersistentSymbolRequest::new(
            PersistentSymbolKey::RootRegistration(self.failure_root),
            scoop_identity::LinkageClass::ConeStrong,
        )
    }

    pub fn gateway_symbol(&self) -> Result<PersistentSymbolRequest, PersistentSymbolError> {
        PersistentSymbolRequest::new(
            PersistentSymbolKey::CallableBody(self.gateway),
            scoop_identity::LinkageClass::ConeStrong,
        )
    }
}

impl WireEncode for ExecutableEntryPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(11)?;
        encoder.field(1)?;
        self.root_cone.encode(encoder)?;
        encoder.field(2)?;
        self.declaration.encode(encoder)?;
        encoder.field(3)?;
        self.source_signature.encode(encoder)?;
        encoder.field(4)?;
        self.source_signature_fingerprint.encode(encoder)?;
        encoder.field(5)?;
        self.main.encode(encoder)?;
        encoder.field(6)?;
        self.failure_root.encode(encoder)?;
        encoder.field(7)?;
        self.gateway.encode(encoder)?;
        encoder.field(8)?;
        self.root_descriptor_symbol.encode(encoder)?;
        encoder.field(9)?;
        self.root_descriptor_definition.encode(encoder)?;
        encoder.field(10)?;
        self.source_signature_patch.encode(encoder)?;
        encoder.field(11)?;
        self.gateway_definition_patch.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntryProductionPlanV1 {
    Library,
    Executable(Box<ExecutableEntryPlanV1>),
}

impl EntryProductionPlanV1 {
    pub fn new(
        source: EntryProductionSourceV1,
        foundation: &ConeLirFoundation,
        registrations: &StrongRegistrationIdentitySurfaceV1,
        digests: &StrongDigestFinalizationPlanV1,
    ) -> Result<Self, EntryProductionPlanBuildError> {
        match source {
            EntryProductionSourceV1::Library => {
                validate_library_foundation(foundation)?;
                Ok(Self::Library)
            }
            EntryProductionSourceV1::Executable(entry) => {
                build_executable(*entry, foundation, registrations, digests)
                    .map(Box::new)
                    .map(Self::Executable)
            }
        }
    }
}

impl WireEncode for EntryProductionPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Library => encode_empty_sum(encoder, 1),
            Self::Executable(plan) => encode_value_sum(encoder, 2, plan.as_ref()),
        }
    }
}

#[derive(Debug)]
pub struct DecodedExecutableEntryPlanV1 {
    root_cone: DecodedPersistentId<scoop_identity::ConeIdentity>,
    declaration: DecodedPersistentId<PersistentFunctionId>,
    source_signature: DecodedExactCallableSignature,
    source_signature_fingerprint: DecodedPersistentId<SourceSignatureFingerprint>,
    main: DecodedPersistentId<PersistentCallableBodyId>,
    failure_root: DecodedPersistentId<PersistentStaticStorageId>,
    gateway: DecodedPersistentId<PersistentCallableBodyId>,
    root_descriptor_symbol: DecodedPersistentSymbolRequest,
    root_descriptor_definition: DecodedPersistentId<ObjectDefinitionPlanId>,
    source_signature_patch: DecodedPersistentId<DigestPatchIntentId>,
    gateway_definition_patch: DecodedPersistentId<DigestPatchIntentId>,
}

impl WireEncode for DecodedExecutableEntryPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(11)?;
        encoder.field(1)?;
        self.root_cone.encode(encoder)?;
        encoder.field(2)?;
        self.declaration.encode(encoder)?;
        encoder.field(3)?;
        self.source_signature.encode(encoder)?;
        encoder.field(4)?;
        self.source_signature_fingerprint.encode(encoder)?;
        encoder.field(5)?;
        self.main.encode(encoder)?;
        encoder.field(6)?;
        self.failure_root.encode(encoder)?;
        encoder.field(7)?;
        self.gateway.encode(encoder)?;
        encoder.field(8)?;
        self.root_descriptor_symbol.encode(encoder)?;
        encoder.field(9)?;
        self.root_descriptor_definition.encode(encoder)?;
        encoder.field(10)?;
        self.source_signature_patch.encode(encoder)?;
        encoder.field(11)?;
        self.gateway_definition_patch.encode(encoder)
    }
}

impl WireDecode for DecodedExecutableEntryPlanV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(11)?;
        Ok(Self {
            root_cone: decoder.field(1, DecodedPersistentId::decode)?,
            declaration: decoder.field(2, DecodedPersistentId::decode)?,
            source_signature: decoder.field(3, DecodedExactCallableSignature::decode)?,
            source_signature_fingerprint: decoder.field(4, DecodedPersistentId::decode)?,
            main: decoder.field(5, DecodedPersistentId::decode)?,
            failure_root: decoder.field(6, DecodedPersistentId::decode)?,
            gateway: decoder.field(7, DecodedPersistentId::decode)?,
            root_descriptor_symbol: decoder.field(8, DecodedPersistentSymbolRequest::decode)?,
            root_descriptor_definition: decoder.field(9, DecodedPersistentId::decode)?,
            source_signature_patch: decoder.field(10, DecodedPersistentId::decode)?,
            gateway_definition_patch: decoder.field(11, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Debug)]
pub enum DecodedEntryProductionPlanV1 {
    Library,
    Executable(Box<DecodedExecutableEntryPlanV1>),
}

impl DecodedEntryProductionPlanV1 {
    pub fn validate(
        self,
        source: EntryProductionSourceV1,
        foundation: &ConeLirFoundation,
        registrations: &StrongRegistrationIdentitySurfaceV1,
        digests: &StrongDigestFinalizationPlanV1,
    ) -> Result<EntryProductionPlanV1, EntryProductionPlanValidationError> {
        let actual = encode(&self).map_err(EntryProductionPlanValidationError::Encode)?;
        let expected = EntryProductionPlanV1::new(source, foundation, registrations, digests)
            .map_err(EntryProductionPlanValidationError::Expected)?;
        let expected_bytes =
            encode(&expected).map_err(EntryProductionPlanValidationError::Encode)?;
        if actual != expected_bytes {
            return Err(EntryProductionPlanValidationError::PlanMismatch);
        }
        Ok(expected)
    }
}

impl WireEncode for DecodedEntryProductionPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Library => encode_empty_sum(encoder, 1),
            Self::Executable(plan) => encode_value_sum(encoder, 2, plan.as_ref()),
        }
    }
}

impl WireDecode for DecodedEntryProductionPlanV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decode_sum(decoder, |tag, decoder| match tag {
            1 => Ok(Self::Library),
            2 => DecodedExecutableEntryPlanV1::decode(decoder)
                .map(Box::new)
                .map(Self::Executable),
            tag => Err(unknown_tag(decoder, tag)),
        })
    }
}

fn decode_sum<T>(
    decoder: &mut Decoder<'_>,
    decode: impl FnOnce(u64, &mut Decoder<'_>) -> Result<T, WireError>,
) -> Result<T, WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    match (tag, fields) {
        (1, 1) => decode(tag, decoder),
        (2, 2) => decoder.field(1, |decoder| decode(tag, decoder)),
        _ if tag > 2 => Err(unknown_tag(decoder, tag)),
        _ => Err(WireError::new(
            scoop_wire::WireErrorKind::InvalidLength {
                expected: if tag == 1 { 1 } else { 2 },
                actual: fields,
            },
            decoder.path().clone(),
            Some(decoder.position()),
        )),
    }
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn unknown_tag(decoder: &Decoder<'_>, tag: u64) -> WireError {
    WireError::new(
        scoop_wire::WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

#[cfg(test)]
mod tests;
