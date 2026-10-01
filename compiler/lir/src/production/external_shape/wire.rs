use super::*;
use scoop_identity::{
    DecodedCallableDefinitionOwner, DecodedPersistentId, IdentityReferenceError,
    PersistentIdResolver, ValidatedIdentityGraph,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedExternalStrongShapeSubjectV1 {
    Callable(DecodedCallableDefinitionOwner),
    Layout(DecodedPersistentId<PersistentLayoutId>),
    Scan(DecodedPersistentId<PersistentScanId>),
    TypeDescriptor(DecodedPersistentId<PersistentExactTypeId>),
    DispatchTable(DecodedPersistentId<PersistentDispatchTableId>),
    TypeRegistration(DecodedPersistentId<PersistentExactTypeId>),
    StaticStorage(DecodedPersistentId<PersistentStaticStorageId>),
    StaticStorageRegistration(DecodedPersistentId<PersistentStaticStorageId>),
    InitializationCell(DecodedPersistentId<PersistentInitializationUnitId>),
    InitializationRegistration(DecodedPersistentId<PersistentInitializationUnitId>),
}

impl DecodedExternalStrongShapeSubjectV1 {
    pub fn resolve(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<ExternalStrongShapeSubjectV1, ExternalShapeSubjectResolutionError> {
        Ok(match self {
            Self::Callable(id) => ExternalStrongShapeSubjectV1::Callable(
                id.resolve(identities)
                    .map_err(ExternalShapeSubjectResolutionError::Callable)?,
            ),
            Self::Layout(id) => ExternalStrongShapeSubjectV1::Layout(
                identities
                    .resolve(id)
                    .map_err(ExternalShapeSubjectResolutionError::Identity)?,
            ),
            Self::Scan(id) => ExternalStrongShapeSubjectV1::Scan(
                identities
                    .resolve(id)
                    .map_err(ExternalShapeSubjectResolutionError::Identity)?,
            ),
            Self::TypeDescriptor(id) => ExternalStrongShapeSubjectV1::TypeDescriptor(
                identities
                    .resolve(id)
                    .map_err(ExternalShapeSubjectResolutionError::Identity)?,
            ),
            Self::DispatchTable(id) => ExternalStrongShapeSubjectV1::DispatchTable(
                identities
                    .resolve(id)
                    .map_err(ExternalShapeSubjectResolutionError::Identity)?,
            ),
            Self::TypeRegistration(id) => ExternalStrongShapeSubjectV1::TypeRegistration(
                identities
                    .resolve(id)
                    .map_err(ExternalShapeSubjectResolutionError::Identity)?,
            ),
            Self::StaticStorage(id) => ExternalStrongShapeSubjectV1::StaticStorage(
                identities
                    .resolve(id)
                    .map_err(ExternalShapeSubjectResolutionError::Identity)?,
            ),
            Self::StaticStorageRegistration(id) => {
                ExternalStrongShapeSubjectV1::StaticStorageRegistration(
                    identities
                        .resolve(id)
                        .map_err(ExternalShapeSubjectResolutionError::Identity)?,
                )
            }
            Self::InitializationCell(id) => ExternalStrongShapeSubjectV1::InitializationCell(
                identities
                    .resolve(id)
                    .map_err(ExternalShapeSubjectResolutionError::Identity)?,
            ),
            Self::InitializationRegistration(id) => {
                ExternalStrongShapeSubjectV1::InitializationRegistration(
                    identities
                        .resolve(id)
                        .map_err(ExternalShapeSubjectResolutionError::Identity)?,
                )
            }
        })
    }
}

impl WireEncode for ExternalStrongShapeSubjectV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Callable(id) => encode_subject(encoder, 1, id),
            Self::Layout(id) => encode_subject(encoder, 2, id),
            Self::Scan(id) => encode_subject(encoder, 3, id),
            Self::TypeDescriptor(id) => encode_subject(encoder, 4, id),
            Self::DispatchTable(id) => encode_subject(encoder, 5, id),
            Self::TypeRegistration(id) => encode_subject(encoder, 6, id),
            Self::StaticStorage(id) => encode_subject(encoder, 7, id),
            Self::StaticStorageRegistration(id) => encode_subject(encoder, 8, id),
            Self::InitializationCell(id) => encode_subject(encoder, 9, id),
            Self::InitializationRegistration(id) => encode_subject(encoder, 11, id),
        }
    }
}

impl WireEncode for DecodedExternalStrongShapeSubjectV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Callable(id) => encode_subject(encoder, 1, id),
            Self::Layout(id) => encode_subject(encoder, 2, id),
            Self::Scan(id) => encode_subject(encoder, 3, id),
            Self::TypeDescriptor(id) => encode_subject(encoder, 4, id),
            Self::DispatchTable(id) => encode_subject(encoder, 5, id),
            Self::TypeRegistration(id) => encode_subject(encoder, 6, id),
            Self::StaticStorage(id) => encode_subject(encoder, 7, id),
            Self::StaticStorageRegistration(id) => encode_subject(encoder, 8, id),
            Self::InitializationCell(id) => encode_subject(encoder, 9, id),
            Self::InitializationRegistration(id) => encode_subject(encoder, 11, id),
        }
    }
}

fn encode_subject(
    encoder: &mut Encoder,
    tag: u64,
    payload: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    payload.encode(encoder)
}

impl WireDecode for DecodedExternalStrongShapeSubjectV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => Ok(Self::Callable(
                decoder.field(1, DecodedCallableDefinitionOwner::decode)?,
            )),
            2 => Ok(Self::Layout(decoder.field(1, DecodedPersistentId::decode)?)),
            3 => Ok(Self::Scan(decoder.field(1, DecodedPersistentId::decode)?)),
            4 => Ok(Self::TypeDescriptor(
                decoder.field(1, DecodedPersistentId::decode)?,
            )),
            5 => Ok(Self::DispatchTable(
                decoder.field(1, DecodedPersistentId::decode)?,
            )),
            6 => Ok(Self::TypeRegistration(
                decoder.field(1, DecodedPersistentId::decode)?,
            )),
            7 => Ok(Self::StaticStorage(
                decoder.field(1, DecodedPersistentId::decode)?,
            )),
            8 => Ok(Self::StaticStorageRegistration(
                decoder.field(1, DecodedPersistentId::decode)?,
            )),
            9 => Ok(Self::InitializationCell(
                decoder.field(1, DecodedPersistentId::decode)?,
            )),
            11 => Ok(Self::InitializationRegistration(
                decoder.field(1, DecodedPersistentId::decode)?,
            )),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

#[derive(Debug)]
pub enum ExternalShapeSubjectResolutionError {
    Callable(scoop_identity::CallableBodyResolutionError<IdentityReferenceError>),
    Resource(WireError),
    Identity(IdentityReferenceError),
}
impl std::fmt::Display for ExternalShapeSubjectResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid external shape subject identity: {self:?}")
    }
}
impl std::error::Error for ExternalShapeSubjectResolutionError {}
