use std::fmt;

use scoop_wire::{Decoder, Encoder, HashError, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{NativeExternAbi, NativeExternalContract, NativeExternalContractRecord};
use crate::{
    CanonicalCAbiLayoutFingerprint, CanonicalCAbiResolutionError,
    CanonicalCAbiSignatureFingerprint, DecodedCanonicalCAbiFunctionSignature,
    DecodedCanonicalCStorageType, DecodedCanonicalScoopAbiFunctionSignature,
    DecodedNativeExternalSymbolKey, DecodedNativeLibraryBinding, DecodedPersistentId,
    NativeExternalContractFingerprint, NativeLinkRequirementId, NativeLinkValidationError,
    PersistentExactTypeId, PersistentIdMismatch, PersistentIdResolver,
    PersistentNativeExternalSymbolId, PersistentSourceNativeExternalContractId,
    ScoopAbiResolutionError, TargetCallingConvention,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedNativeExternAbi {
    C(DecodedCanonicalCAbiFunctionSignature),
    Scoop(DecodedCanonicalScoopAbiFunctionSignature),
}

impl DecodedNativeExternAbi {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<NativeExternAbi, NativeExternalContractResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<CanonicalCAbiSignatureFingerprint, Error = E>
            + PersistentIdResolver<CanonicalCAbiLayoutFingerprint, Error = E>,
    {
        match self {
            Self::C(signature) => signature
                .resolve(resolver)
                .map(NativeExternAbi::C)
                .map_err(NativeExternalContractResolutionError::CAbi),
            Self::Scoop(signature) => signature
                .resolve(resolver)
                .map(NativeExternAbi::Scoop)
                .map_err(NativeExternalContractResolutionError::ScoopAbi),
        }
    }
}

impl WireEncode for DecodedNativeExternAbi {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::C(signature) => encode_value_sum(encoder, 1, signature),
            Self::Scoop(signature) => encode_value_sum(encoder, 2, signature),
        }
    }
}

impl WireDecode for DecodedNativeExternAbi {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        expect_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder
                .field(1, DecodedCanonicalCAbiFunctionSignature::decode)
                .map(Self::C),
            2 => decoder
                .field(1, DecodedCanonicalScoopAbiFunctionSignature::decode)
                .map(Self::Scoop),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedNativeExternalContract {
    Function {
        library: DecodedNativeLibraryBinding,
        abi: DecodedNativeExternAbi,
        calling_convention: TargetCallingConvention,
    },
    ReadOnlyData {
        library: DecodedNativeLibraryBinding,
        storage: DecodedCanonicalCStorageType,
    },
    MutableData {
        library: DecodedNativeLibraryBinding,
        storage: DecodedCanonicalCStorageType,
    },
    ReadOnlyTls {
        library: DecodedNativeLibraryBinding,
        storage: DecodedCanonicalCStorageType,
    },
    MutableTls {
        library: DecodedNativeLibraryBinding,
        storage: DecodedCanonicalCStorageType,
    },
}

impl DecodedNativeExternalContract {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<NativeExternalContract, NativeExternalContractResolutionError<E>>
    where
        R: PersistentIdResolver<NativeLinkRequirementId, Error = E>
            + PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<CanonicalCAbiSignatureFingerprint, Error = E>
            + PersistentIdResolver<CanonicalCAbiLayoutFingerprint, Error = E>,
    {
        match self {
            Self::Function {
                library,
                abi,
                calling_convention,
            } => {
                let library = library
                    .resolve(resolver)
                    .map_err(NativeExternalContractResolutionError::Reference)?;
                match abi.resolve(resolver)? {
                    NativeExternAbi::C(signature) => {
                        if signature.calling_convention() != calling_convention {
                            return Err(
                                NativeExternalContractResolutionError::CallingConventionMismatch,
                            );
                        }
                        Ok(NativeExternalContract::c_function(library, signature))
                    }
                    NativeExternAbi::Scoop(signature) => {
                        Ok(NativeExternalContract::scoop_function(
                            library,
                            signature,
                            calling_convention,
                        ))
                    }
                }
            }
            Self::ReadOnlyData { library, storage } => resolve_data_contract(
                resolver,
                library,
                storage,
                NativeExternalContract::read_only_data,
            ),
            Self::MutableData { library, storage } => resolve_data_contract(
                resolver,
                library,
                storage,
                NativeExternalContract::mutable_data,
            ),
            Self::ReadOnlyTls { library, storage } => resolve_data_contract(
                resolver,
                library,
                storage,
                NativeExternalContract::read_only_tls,
            ),
            Self::MutableTls { library, storage } => resolve_data_contract(
                resolver,
                library,
                storage,
                NativeExternalContract::mutable_tls,
            ),
        }
    }
}

impl WireEncode for DecodedNativeExternalContract {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function {
                library,
                abi,
                calling_convention,
            } => {
                encoder.map(4)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                library.encode(encoder)?;
                encoder.field(2)?;
                abi.encode(encoder)?;
                encoder.field(3)?;
                calling_convention.encode(encoder)
            }
            Self::ReadOnlyData { library, storage } => {
                encode_data_contract(encoder, 2, library, storage)
            }
            Self::MutableData { library, storage } => {
                encode_data_contract(encoder, 3, library, storage)
            }
            Self::ReadOnlyTls { library, storage } => {
                encode_data_contract(encoder, 4, library, storage)
            }
            Self::MutableTls { library, storage } => {
                encode_data_contract(encoder, 5, library, storage)
            }
        }
    }
}

impl WireDecode for DecodedNativeExternalContract {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::Function {
                    library: decoder.field(1, DecodedNativeLibraryBinding::decode)?,
                    abi: decoder.field(2, DecodedNativeExternAbi::decode)?,
                    calling_convention: decoder.field(3, TargetCallingConvention::decode)?,
                })
            }
            2 => {
                let (library, storage) = decode_data_contract(decoder, fields)?;
                Ok(Self::ReadOnlyData { library, storage })
            }
            3 => {
                let (library, storage) = decode_data_contract(decoder, fields)?;
                Ok(Self::MutableData { library, storage })
            }
            4 => {
                let (library, storage) = decode_data_contract(decoder, fields)?;
                Ok(Self::ReadOnlyTls { library, storage })
            }
            5 => {
                let (library, storage) = decode_data_contract(decoder, fields)?;
                Ok(Self::MutableTls { library, storage })
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNativeExternalContractRecord {
    source: DecodedPersistentId<PersistentSourceNativeExternalContractId>,
    symbol_id: DecodedPersistentId<PersistentNativeExternalSymbolId>,
    symbol_key: DecodedNativeExternalSymbolKey,
    fingerprint: DecodedPersistentId<NativeExternalContractFingerprint>,
    contract: DecodedNativeExternalContract,
}

impl DecodedNativeExternalContractRecord {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<NativeExternalContractRecord, NativeExternalContractResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentSourceNativeExternalContractId, Error = E>
            + PersistentIdResolver<NativeLinkRequirementId, Error = E>
            + PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<CanonicalCAbiSignatureFingerprint, Error = E>
            + PersistentIdResolver<CanonicalCAbiLayoutFingerprint, Error = E>,
    {
        let source = resolver
            .resolve(self.source)
            .map_err(NativeExternalContractResolutionError::Reference)?;
        let symbol_key = self
            .symbol_key
            .validate()
            .map_err(NativeExternalContractResolutionError::SymbolKey)?;
        let contract = self.contract.resolve(resolver)?;
        let record = NativeExternalContractRecord::new(source, symbol_key, contract)
            .map_err(NativeExternalContractResolutionError::Hash)?;
        self.symbol_id
            .verify(record.symbol_id)
            .map_err(NativeExternalContractResolutionError::SymbolId)?;
        self.fingerprint
            .verify(record.fingerprint)
            .map_err(NativeExternalContractResolutionError::Fingerprint)?;
        Ok(record)
    }
}

impl WireEncode for DecodedNativeExternalContractRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.source.encode(encoder)?;
        encoder.field(2)?;
        self.symbol_id.encode(encoder)?;
        encoder.field(3)?;
        self.symbol_key.encode(encoder)?;
        encoder.field(4)?;
        self.fingerprint.encode(encoder)?;
        encoder.field(5)?;
        self.contract.encode(encoder)
    }
}

impl WireDecode for DecodedNativeExternalContractRecord {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            source: decoder.field(1, DecodedPersistentId::decode)?,
            symbol_id: decoder.field(2, DecodedPersistentId::decode)?,
            symbol_key: decoder.field(3, DecodedNativeExternalSymbolKey::decode)?,
            fingerprint: decoder.field(4, DecodedPersistentId::decode)?,
            contract: decoder.field(5, DecodedNativeExternalContract::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeExternalContractResolutionError<E> {
    Reference(E),
    CAbi(CanonicalCAbiResolutionError<E>),
    ScoopAbi(ScoopAbiResolutionError<E>),
    SymbolKey(NativeLinkValidationError),
    CallingConventionMismatch,
    Hash(HashError),
    SymbolId(PersistentIdMismatch<PersistentNativeExternalSymbolId>),
    Fingerprint(PersistentIdMismatch<NativeExternalContractFingerprint>),
}

impl<E: fmt::Display> fmt::Display for NativeExternalContractResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::CAbi(error) => error.fmt(formatter),
            Self::ScoopAbi(error) => error.fmt(formatter),
            Self::SymbolKey(error) => error.fmt(formatter),
            Self::CallingConventionMismatch => formatter.write_str(
                "native C contract calling convention does not match its canonical signature",
            ),
            Self::Hash(error) => error.fmt(formatter),
            Self::SymbolId(error) => error.fmt(formatter),
            Self::Fingerprint(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for NativeExternalContractResolutionError<E>
{
}

fn resolve_data_contract<R, E>(
    resolver: &mut R,
    library: DecodedNativeLibraryBinding,
    storage: DecodedCanonicalCStorageType,
    build: fn(crate::NativeLibraryBinding, crate::CanonicalCStorageType) -> NativeExternalContract,
) -> Result<NativeExternalContract, NativeExternalContractResolutionError<E>>
where
    R: PersistentIdResolver<NativeLinkRequirementId, Error = E>
        + PersistentIdResolver<PersistentExactTypeId, Error = E>
        + PersistentIdResolver<CanonicalCAbiSignatureFingerprint, Error = E>
        + PersistentIdResolver<CanonicalCAbiLayoutFingerprint, Error = E>,
{
    let library = library
        .resolve(resolver)
        .map_err(NativeExternalContractResolutionError::Reference)?;
    let storage = storage
        .resolve(resolver)
        .map_err(NativeExternalContractResolutionError::Reference)?;
    Ok(build(library, storage))
}

fn decode_data_contract(
    decoder: &mut Decoder<'_, '_>,
    fields: u64,
) -> Result<(DecodedNativeLibraryBinding, DecodedCanonicalCStorageType), WireError> {
    expect_sum_length(decoder, fields, 3)?;
    Ok((
        decoder.field(1, DecodedNativeLibraryBinding::decode)?,
        decoder.field(2, DecodedCanonicalCStorageType::decode)?,
    ))
}

fn encode_data_contract(
    encoder: &mut Encoder,
    tag: u64,
    library: &DecodedNativeLibraryBinding,
    storage: &DecodedCanonicalCStorageType,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    library.encode(encoder)?;
    encoder.field(2)?;
    storage.encode(encoder)
}

fn decode_sum_header(decoder: &mut Decoder<'_, '_>) -> Result<(u64, u64), WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    Ok((fields, tag))
}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(WireError::new(
            WireErrorKind::InvalidLength { expected, actual },
            decoder.path().clone(),
            Some(decoder.position()),
        ))
    }
}

fn unknown_tag(decoder: &Decoder<'_, '_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

#[cfg(test)]
mod tests;
