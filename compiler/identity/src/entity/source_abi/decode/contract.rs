use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    DecodedSourceExternFunctionAbi, DecodedSourceNativeLibraryBinding, decode_sum_header,
    encode_tag, expect_sum_length, unknown_tag,
};
use crate::{
    CanonicalNativeNameError, DecodedPersistentId, DecodedSignatureTypeKey,
    DecodedSourceNativeSymbol, PersistentFunctionId, PersistentGenericTypeId, PersistentIdMismatch,
    PersistentIdResolver, PersistentKeyResolver, PersistentPropertyId,
    PersistentSourceNativeExternalContractId, PersistentTypeId, SignatureTypeKey,
    SourceCallingConvention, SourceDeclarationKey, SourceNativeContractError,
    SourceNativeExternalContract, SourceNativeExternalContractKey,
    SourceNativeExternalContractRecord, SourceNativeLibraryBinding, SourceNativeSymbol,
    SourceNativeSymbolError,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedSourceNativeExternalOwner {
    Function(DecodedPersistentId<PersistentFunctionId>),
    Property(DecodedPersistentId<PersistentPropertyId>),
}

impl WireEncode for DecodedSourceNativeExternalOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_value_sum(encoder, 1, id),
            Self::Property(id) => encode_value_sum(encoder, 2, id),
        }
    }
}

impl WireDecode for DecodedSourceNativeExternalOwner {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        expect_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Function),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Property),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedSourceNativeExternalContractKey {
    owner: DecodedSourceNativeExternalOwner,
}

impl DecodedSourceNativeExternalContractKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<SourceNativeExternalContractKey, SourceNativeExternalResolutionError<E>>
    where
        R: PersistentKeyResolver<PersistentFunctionId, SourceDeclarationKey, Error = E>
            + PersistentKeyResolver<PersistentPropertyId, SourceDeclarationKey, Error = E>,
    {
        match self.owner {
            DecodedSourceNativeExternalOwner::Function(id) => {
                resolve_owner_key(resolver, id, SourceNativeExternalContractKey::function)
            }
            DecodedSourceNativeExternalOwner::Property(id) => {
                resolve_owner_key(resolver, id, SourceNativeExternalContractKey::property)
            }
        }
    }
}

impl WireEncode for DecodedSourceNativeExternalContractKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(1)?;
        self.owner.encode(encoder)
    }
}

impl WireDecode for DecodedSourceNativeExternalContractKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        Ok(Self {
            owner: decoder.field(1, DecodedSourceNativeExternalOwner::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedSourceNativeExternalContract {
    Function {
        symbol: DecodedSourceNativeSymbol,
        library: DecodedSourceNativeLibraryBinding,
        abi: DecodedSourceExternFunctionAbi,
        calling_convention: SourceCallingConvention,
    },
    ReadOnlyData {
        symbol: DecodedSourceNativeSymbol,
        library: DecodedSourceNativeLibraryBinding,
        storage: DecodedSignatureTypeKey,
    },
    MutableData {
        symbol: DecodedSourceNativeSymbol,
        library: DecodedSourceNativeLibraryBinding,
        storage: DecodedSignatureTypeKey,
    },
    ReadOnlyTls {
        symbol: DecodedSourceNativeSymbol,
        library: DecodedSourceNativeLibraryBinding,
        storage: DecodedSignatureTypeKey,
    },
    MutableTls {
        symbol: DecodedSourceNativeSymbol,
        library: DecodedSourceNativeLibraryBinding,
        storage: DecodedSignatureTypeKey,
    },
}

impl DecodedSourceNativeExternalContract {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<SourceNativeExternalContract, SourceNativeExternalResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentGenericTypeId, Error = E>,
    {
        match self {
            Self::Function {
                symbol,
                library,
                abi,
                calling_convention,
            } => Ok(SourceNativeExternalContract::Function {
                symbol: resolve_symbol(symbol)?,
                library: resolve_library(library)?,
                abi: abi
                    .resolve(resolver)
                    .map_err(SourceNativeExternalResolutionError::Reference)?,
                calling_convention,
            }),
            Self::ReadOnlyData {
                symbol,
                library,
                storage,
            } => resolve_data_contract(
                resolver,
                symbol,
                library,
                storage,
                |symbol, library, storage| SourceNativeExternalContract::ReadOnlyData {
                    symbol,
                    library,
                    storage,
                },
            ),
            Self::MutableData {
                symbol,
                library,
                storage,
            } => resolve_data_contract(
                resolver,
                symbol,
                library,
                storage,
                |symbol, library, storage| SourceNativeExternalContract::MutableData {
                    symbol,
                    library,
                    storage,
                },
            ),
            Self::ReadOnlyTls {
                symbol,
                library,
                storage,
            } => resolve_data_contract(
                resolver,
                symbol,
                library,
                storage,
                |symbol, library, storage| SourceNativeExternalContract::ReadOnlyTls {
                    symbol,
                    library,
                    storage,
                },
            ),
            Self::MutableTls {
                symbol,
                library,
                storage,
            } => resolve_data_contract(
                resolver,
                symbol,
                library,
                storage,
                |symbol, library, storage| SourceNativeExternalContract::MutableTls {
                    symbol,
                    library,
                    storage,
                },
            ),
        }
    }
}

impl WireEncode for DecodedSourceNativeExternalContract {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function {
                symbol,
                library,
                abi,
                calling_convention,
            } => {
                encoder.map(5)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                symbol.encode(encoder)?;
                encoder.field(2)?;
                library.encode(encoder)?;
                encoder.field(3)?;
                abi.encode(encoder)?;
                encoder.field(4)?;
                calling_convention.encode(encoder)
            }
            Self::ReadOnlyData {
                symbol,
                library,
                storage,
            } => encode_data_contract(encoder, 2, symbol, library, storage),
            Self::MutableData {
                symbol,
                library,
                storage,
            } => encode_data_contract(encoder, 3, symbol, library, storage),
            Self::ReadOnlyTls {
                symbol,
                library,
                storage,
            } => encode_data_contract(encoder, 4, symbol, library, storage),
            Self::MutableTls {
                symbol,
                library,
                storage,
            } => encode_data_contract(encoder, 5, symbol, library, storage),
        }
    }
}

impl WireDecode for DecodedSourceNativeExternalContract {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 5)?;
                Ok(Self::Function {
                    symbol: decoder.field(1, DecodedSourceNativeSymbol::decode)?,
                    library: decoder.field(2, DecodedSourceNativeLibraryBinding::decode)?,
                    abi: decoder.field(3, DecodedSourceExternFunctionAbi::decode)?,
                    calling_convention: decoder.field(4, SourceCallingConvention::decode)?,
                })
            }
            2 => {
                let (symbol, library, storage) = decode_data_contract(decoder, fields)?;
                Ok(Self::ReadOnlyData {
                    symbol,
                    library,
                    storage,
                })
            }
            3 => {
                let (symbol, library, storage) = decode_data_contract(decoder, fields)?;
                Ok(Self::MutableData {
                    symbol,
                    library,
                    storage,
                })
            }
            4 => {
                let (symbol, library, storage) = decode_data_contract(decoder, fields)?;
                Ok(Self::ReadOnlyTls {
                    symbol,
                    library,
                    storage,
                })
            }
            5 => {
                let (symbol, library, storage) = decode_data_contract(decoder, fields)?;
                Ok(Self::MutableTls {
                    symbol,
                    library,
                    storage,
                })
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedSourceNativeExternalContractRecord {
    id: DecodedPersistentId<PersistentSourceNativeExternalContractId>,
    key: DecodedSourceNativeExternalContractKey,
    contract: DecodedSourceNativeExternalContract,
}

impl DecodedSourceNativeExternalContractRecord {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<SourceNativeExternalContractRecord, SourceNativeExternalResolutionError<E>>
    where
        R: PersistentKeyResolver<PersistentFunctionId, SourceDeclarationKey, Error = E>
            + PersistentKeyResolver<PersistentPropertyId, SourceDeclarationKey, Error = E>
            + PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentGenericTypeId, Error = E>,
    {
        let key = self.key.resolve(resolver)?;
        let contract = self.contract.resolve(resolver)?;
        let record = SourceNativeExternalContractRecord::new(key, contract)
            .map_err(SourceNativeExternalResolutionError::Contract)?;
        self.id
            .verify(record.id())
            .map_err(SourceNativeExternalResolutionError::Id)?;
        Ok(record)
    }
}

impl WireEncode for DecodedSourceNativeExternalContractRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.id.encode(encoder)?;
        encoder.field(2)?;
        self.key.encode(encoder)?;
        encoder.field(3)?;
        self.contract.encode(encoder)
    }
}

impl WireDecode for DecodedSourceNativeExternalContractRecord {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            id: decoder.field(1, DecodedPersistentId::decode)?,
            key: decoder.field(2, DecodedSourceNativeExternalContractKey::decode)?,
            contract: decoder.field(3, DecodedSourceNativeExternalContract::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceNativeExternalResolutionError<E> {
    Reference(E),
    Library(CanonicalNativeNameError),
    Symbol(SourceNativeSymbolError),
    Contract(SourceNativeContractError),
    Id(PersistentIdMismatch<PersistentSourceNativeExternalContractId>),
}

impl<E: fmt::Display> fmt::Display for SourceNativeExternalResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Library(error) => error.fmt(formatter),
            Self::Symbol(error) => error.fmt(formatter),
            Self::Contract(error) => error.fmt(formatter),
            Self::Id(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SourceNativeExternalResolutionError<E> {}

fn resolve_owner_key<R, I, E>(
    resolver: &mut R,
    id: DecodedPersistentId<I>,
    build: fn(
        &SourceDeclarationKey,
    ) -> Result<SourceNativeExternalContractKey, SourceNativeContractError>,
) -> Result<SourceNativeExternalContractKey, SourceNativeExternalResolutionError<E>>
where
    I: crate::PersistentId,
    R: PersistentKeyResolver<I, SourceDeclarationKey, Error = E>,
{
    let key = resolver
        .resolve_key(id)
        .map_err(SourceNativeExternalResolutionError::Reference)?;
    build(&key).map_err(SourceNativeExternalResolutionError::Contract)
}

fn resolve_symbol<E>(
    symbol: DecodedSourceNativeSymbol,
) -> Result<SourceNativeSymbol, SourceNativeExternalResolutionError<E>> {
    symbol
        .validate()
        .map_err(SourceNativeExternalResolutionError::Symbol)
}

fn resolve_library<E>(
    library: DecodedSourceNativeLibraryBinding,
) -> Result<SourceNativeLibraryBinding, SourceNativeExternalResolutionError<E>> {
    library
        .validate()
        .map_err(SourceNativeExternalResolutionError::Library)
}

fn resolve_data_contract<R, E>(
    resolver: &mut R,
    symbol: DecodedSourceNativeSymbol,
    library: DecodedSourceNativeLibraryBinding,
    storage: DecodedSignatureTypeKey,
    build: fn(
        SourceNativeSymbol,
        SourceNativeLibraryBinding,
        SignatureTypeKey,
    ) -> SourceNativeExternalContract,
) -> Result<SourceNativeExternalContract, SourceNativeExternalResolutionError<E>>
where
    R: PersistentIdResolver<PersistentTypeId, Error = E>
        + PersistentIdResolver<PersistentGenericTypeId, Error = E>,
{
    Ok(build(
        resolve_symbol(symbol)?,
        resolve_library(library)?,
        storage
            .resolve(resolver)
            .map_err(SourceNativeExternalResolutionError::Reference)?,
    ))
}

fn decode_data_contract(
    decoder: &mut Decoder<'_, '_>,
    fields: u64,
) -> Result<
    (
        DecodedSourceNativeSymbol,
        DecodedSourceNativeLibraryBinding,
        DecodedSignatureTypeKey,
    ),
    WireError,
> {
    expect_sum_length(decoder, fields, 4)?;
    Ok((
        decoder.field(1, DecodedSourceNativeSymbol::decode)?,
        decoder.field(2, DecodedSourceNativeLibraryBinding::decode)?,
        decoder.field(3, DecodedSignatureTypeKey::decode)?,
    ))
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

fn encode_data_contract(
    encoder: &mut Encoder,
    tag: u64,
    symbol: &DecodedSourceNativeSymbol,
    library: &DecodedSourceNativeLibraryBinding,
    storage: &DecodedSignatureTypeKey,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(4)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    symbol.encode(encoder)?;
    encoder.field(2)?;
    library.encode(encoder)?;
    encoder.field(3)?;
    storage.encode(encoder)
}

#[cfg(test)]
mod tests;
