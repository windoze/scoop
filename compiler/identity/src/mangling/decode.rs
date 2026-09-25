use std::cmp::Ordering;
use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    LinkageClass, MANGLING_SCHEMA_NAME, ManglingSchemaIdentity, PersistentSymbolError,
    PersistentSymbolKey, PersistentSymbolKind, PersistentSymbolRequest,
    PersistentSymbolRequestTable,
};
use crate::{
    ConeIdentity, DecodedPersistentId, GeneratedBridgeAtomId, ObjectDefinitionAtomId, OdrMemberId,
    PersistentCallableBodyId, PersistentDispatchSlotId, PersistentDispatchTableId,
    PersistentExactTypeId, PersistentIdResolver, PersistentImmortalObjectId,
    PersistentInitializationUnitId, PersistentLayoutId, PersistentSafepointSiteId,
    PersistentScanId, PersistentStaticStorageId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedManglingSchemaIdentity(String);

impl DecodedManglingSchemaIdentity {
    pub fn resolve(self) -> Result<ManglingSchemaIdentity, ManglingSchemaIdentityError> {
        if self.0 == MANGLING_SCHEMA_NAME {
            Ok(ManglingSchemaIdentity)
        } else {
            Err(ManglingSchemaIdentityError::UnknownIdentity(self.0))
        }
    }
}

impl WireEncode for DecodedManglingSchemaIdentity {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.text(&self.0)
    }
}

impl WireDecode for DecodedManglingSchemaIdentity {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.owned_text().map(Self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManglingSchemaIdentityError {
    UnknownIdentity(String),
}

impl fmt::Display for ManglingSchemaIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownIdentity(identity) => {
                write!(formatter, "unknown mangling schema identity {identity:?}")
            }
        }
    }
}

impl std::error::Error for ManglingSchemaIdentityError {}

pub trait PersistentSymbolResolver<E>:
    PersistentIdResolver<PersistentCallableBodyId, Error = E>
    + PersistentIdResolver<PersistentStaticStorageId, Error = E>
    + PersistentIdResolver<PersistentImmortalObjectId, Error = E>
    + PersistentIdResolver<PersistentExactTypeId, Error = E>
    + PersistentIdResolver<PersistentLayoutId, Error = E>
    + PersistentIdResolver<PersistentScanId, Error = E>
    + PersistentIdResolver<PersistentDispatchTableId, Error = E>
    + PersistentIdResolver<PersistentDispatchSlotId, Error = E>
    + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
    + PersistentIdResolver<PersistentSafepointSiteId, Error = E>
    + PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentIdResolver<GeneratedBridgeAtomId, Error = E>
    + PersistentIdResolver<OdrMemberId, Error = E>
    + PersistentIdResolver<ObjectDefinitionAtomId, Error = E>
{
}

impl<T, E> PersistentSymbolResolver<E> for T where
    T: PersistentIdResolver<PersistentCallableBodyId, Error = E>
        + PersistentIdResolver<PersistentStaticStorageId, Error = E>
        + PersistentIdResolver<PersistentImmortalObjectId, Error = E>
        + PersistentIdResolver<PersistentExactTypeId, Error = E>
        + PersistentIdResolver<PersistentLayoutId, Error = E>
        + PersistentIdResolver<PersistentScanId, Error = E>
        + PersistentIdResolver<PersistentDispatchTableId, Error = E>
        + PersistentIdResolver<PersistentDispatchSlotId, Error = E>
        + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
        + PersistentIdResolver<PersistentSafepointSiteId, Error = E>
        + PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentIdResolver<GeneratedBridgeAtomId, Error = E>
        + PersistentIdResolver<OdrMemberId, Error = E>
        + PersistentIdResolver<ObjectDefinitionAtomId, Error = E>
{
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedPersistentSymbolKey {
    CallableBody(DecodedPersistentId<PersistentCallableBodyId>),
    StaticStorage(DecodedPersistentId<PersistentStaticStorageId>),
    ImmortalObject(DecodedPersistentId<PersistentImmortalObjectId>),
    TypeDescriptor(DecodedPersistentId<PersistentExactTypeId>),
    Layout(DecodedPersistentId<PersistentLayoutId>),
    ScanProgram(DecodedPersistentId<PersistentScanId>),
    DispatchTable(DecodedPersistentId<PersistentDispatchTableId>),
    DispatchSlot(DecodedPersistentId<PersistentDispatchSlotId>),
    InitializationCell(DecodedPersistentId<PersistentInitializationUnitId>),
    InitializationDescriptor(DecodedPersistentId<PersistentInitializationUnitId>),
    RootRegistration(DecodedPersistentId<PersistentStaticStorageId>),
    ImmortalRegistration(DecodedPersistentId<PersistentImmortalObjectId>),
    InitializationRegistration(DecodedPersistentId<PersistentInitializationUnitId>),
    TypeRegistration(DecodedPersistentId<PersistentExactTypeId>),
    SafepointRegistration(DecodedPersistentId<PersistentSafepointSiteId>),
    CallableRegistration(DecodedPersistentId<PersistentCallableBodyId>),
    ImageDescriptor(DecodedPersistentId<ConeIdentity>),
    GeneratedBridge(DecodedPersistentId<GeneratedBridgeAtomId>),
    OdrMember(DecodedPersistentId<OdrMemberId>),
    DefinitionBoundaryStart(DecodedPersistentId<ObjectDefinitionAtomId>),
    DefinitionBoundaryEnd(DecodedPersistentId<ObjectDefinitionAtomId>),
    RootEntryDescriptor(DecodedPersistentId<ConeIdentity>),
}

impl DecodedPersistentSymbolKey {
    pub const fn kind(self) -> PersistentSymbolKind {
        match self {
            Self::CallableBody(_) => PersistentSymbolKind::CallableBody,
            Self::StaticStorage(_) => PersistentSymbolKind::StaticStorage,
            Self::ImmortalObject(_) => PersistentSymbolKind::ImmortalObject,
            Self::TypeDescriptor(_) => PersistentSymbolKind::TypeDescriptor,
            Self::Layout(_) => PersistentSymbolKind::Layout,
            Self::ScanProgram(_) => PersistentSymbolKind::ScanProgram,
            Self::DispatchTable(_) => PersistentSymbolKind::DispatchTable,
            Self::DispatchSlot(_) => PersistentSymbolKind::DispatchSlot,
            Self::InitializationCell(_) => PersistentSymbolKind::InitializationCell,
            Self::InitializationDescriptor(_) => PersistentSymbolKind::InitializationDescriptor,
            Self::RootRegistration(_) => PersistentSymbolKind::RootRegistration,
            Self::ImmortalRegistration(_) => PersistentSymbolKind::ImmortalRegistration,
            Self::InitializationRegistration(_) => PersistentSymbolKind::InitializationRegistration,
            Self::TypeRegistration(_) => PersistentSymbolKind::TypeRegistration,
            Self::SafepointRegistration(_) => PersistentSymbolKind::SafepointRegistration,
            Self::CallableRegistration(_) => PersistentSymbolKind::CallableRegistration,
            Self::ImageDescriptor(_) => PersistentSymbolKind::ImageDescriptor,
            Self::GeneratedBridge(_) => PersistentSymbolKind::GeneratedBridge,
            Self::OdrMember(_) => PersistentSymbolKind::OdrMember,
            Self::DefinitionBoundaryStart(_) => PersistentSymbolKind::DefinitionBoundaryStart,
            Self::DefinitionBoundaryEnd(_) => PersistentSymbolKind::DefinitionBoundaryEnd,
            Self::RootEntryDescriptor(_) => PersistentSymbolKind::RootEntryDescriptor,
        }
    }

    pub fn owner_bytes(&self) -> &[u8; 32] {
        match self {
            Self::CallableBody(id) | Self::CallableRegistration(id) => id.as_array(),
            Self::StaticStorage(id) | Self::RootRegistration(id) => id.as_array(),
            Self::ImmortalObject(id) | Self::ImmortalRegistration(id) => id.as_array(),
            Self::TypeDescriptor(id) | Self::TypeRegistration(id) => id.as_array(),
            Self::Layout(id) => id.as_array(),
            Self::ScanProgram(id) => id.as_array(),
            Self::DispatchTable(id) => id.as_array(),
            Self::DispatchSlot(id) => id.as_array(),
            Self::InitializationCell(id)
            | Self::InitializationDescriptor(id)
            | Self::InitializationRegistration(id) => id.as_array(),
            Self::SafepointRegistration(id) => id.as_array(),
            Self::ImageDescriptor(id) => id.as_array(),
            Self::GeneratedBridge(id) => id.as_array(),
            Self::OdrMember(id) => id.as_array(),
            Self::DefinitionBoundaryStart(id) | Self::DefinitionBoundaryEnd(id) => id.as_array(),
            Self::RootEntryDescriptor(id) => id.as_array(),
        }
    }

    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<PersistentSymbolKey, E>
    where
        R: PersistentSymbolResolver<E>,
    {
        match self {
            Self::CallableBody(id) => resolver.resolve(id).map(PersistentSymbolKey::CallableBody),
            Self::StaticStorage(id) => resolver.resolve(id).map(PersistentSymbolKey::StaticStorage),
            Self::ImmortalObject(id) => resolver
                .resolve(id)
                .map(PersistentSymbolKey::ImmortalObject),
            Self::TypeDescriptor(id) => resolver
                .resolve(id)
                .map(PersistentSymbolKey::TypeDescriptor),
            Self::Layout(id) => resolver.resolve(id).map(PersistentSymbolKey::Layout),
            Self::ScanProgram(id) => resolver.resolve(id).map(PersistentSymbolKey::ScanProgram),
            Self::DispatchTable(id) => resolver.resolve(id).map(PersistentSymbolKey::DispatchTable),
            Self::DispatchSlot(id) => resolver.resolve(id).map(PersistentSymbolKey::DispatchSlot),
            Self::InitializationCell(id) => resolver
                .resolve(id)
                .map(PersistentSymbolKey::InitializationCell),
            Self::InitializationDescriptor(id) => resolver
                .resolve(id)
                .map(PersistentSymbolKey::InitializationDescriptor),
            Self::RootRegistration(id) => resolver
                .resolve(id)
                .map(PersistentSymbolKey::RootRegistration),
            Self::ImmortalRegistration(id) => resolver
                .resolve(id)
                .map(PersistentSymbolKey::ImmortalRegistration),
            Self::InitializationRegistration(id) => resolver
                .resolve(id)
                .map(PersistentSymbolKey::InitializationRegistration),
            Self::TypeRegistration(id) => resolver
                .resolve(id)
                .map(PersistentSymbolKey::TypeRegistration),
            Self::SafepointRegistration(id) => resolver
                .resolve(id)
                .map(PersistentSymbolKey::SafepointRegistration),
            Self::CallableRegistration(id) => resolver
                .resolve(id)
                .map(PersistentSymbolKey::CallableRegistration),
            Self::ImageDescriptor(id) => resolver
                .resolve(id)
                .map(PersistentSymbolKey::ImageDescriptor),
            Self::GeneratedBridge(id) => resolver
                .resolve(id)
                .map(PersistentSymbolKey::GeneratedBridge),
            Self::OdrMember(id) => resolver.resolve(id).map(PersistentSymbolKey::OdrMember),
            Self::DefinitionBoundaryStart(id) => resolver
                .resolve(id)
                .map(PersistentSymbolKey::DefinitionBoundaryStart),
            Self::DefinitionBoundaryEnd(id) => resolver
                .resolve(id)
                .map(PersistentSymbolKey::DefinitionBoundaryEnd),
            Self::RootEntryDescriptor(id) => resolver
                .resolve(id)
                .map(PersistentSymbolKey::RootEntryDescriptor),
        }
    }
}

impl WireEncode for DecodedPersistentSymbolKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(self.kind().tag())?;
        encoder.field(1)?;
        encoder.bytes(self.owner_bytes())
    }
}

impl WireDecode for DecodedPersistentSymbolKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => decode_id(decoder, Self::CallableBody),
            2 => decode_id(decoder, Self::StaticStorage),
            3 => decode_id(decoder, Self::ImmortalObject),
            4 => decode_id(decoder, Self::TypeDescriptor),
            5 => decode_id(decoder, Self::Layout),
            6 => decode_id(decoder, Self::ScanProgram),
            7 => decode_id(decoder, Self::DispatchTable),
            8 => decode_id(decoder, Self::DispatchSlot),
            9 => decode_id(decoder, Self::InitializationCell),
            10 => decode_id(decoder, Self::InitializationDescriptor),
            11 => decode_id(decoder, Self::RootRegistration),
            12 => decode_id(decoder, Self::ImmortalRegistration),
            13 => decode_id(decoder, Self::InitializationRegistration),
            14 => decode_id(decoder, Self::TypeRegistration),
            15 => decode_id(decoder, Self::SafepointRegistration),
            16 => decode_id(decoder, Self::CallableRegistration),
            17 => decode_id(decoder, Self::ImageDescriptor),
            18 => decode_id(decoder, Self::GeneratedBridge),
            19 => decode_id(decoder, Self::OdrMember),
            20 => decode_id(decoder, Self::DefinitionBoundaryStart),
            21 => decode_id(decoder, Self::DefinitionBoundaryEnd),
            22 => decode_id(decoder, Self::RootEntryDescriptor),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for LinkageClass {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::ConeStrong),
            2 => Ok(Self::TemplateSupportHidden),
            3 => Ok(Self::OdrWeak),
            4 => Ok(Self::RuntimeAbi),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedPersistentSymbolRequest {
    key: DecodedPersistentSymbolKey,
    linkage: LinkageClass,
}

impl DecodedPersistentSymbolRequest {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<PersistentSymbolRequest, PersistentSymbolResolutionError<E>>
    where
        R: PersistentSymbolResolver<E>,
    {
        let key = self
            .key
            .resolve(resolver)
            .map_err(PersistentSymbolResolutionError::Reference)?;
        PersistentSymbolRequest::new(key, self.linkage)
            .map_err(PersistentSymbolResolutionError::Symbol)
    }
}

impl WireEncode for DecodedPersistentSymbolRequest {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.key.encode(encoder)?;
        encoder.field(2)?;
        self.linkage.encode(encoder)
    }
}

impl WireDecode for DecodedPersistentSymbolRequest {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            key: decoder.field(1, DecodedPersistentSymbolKey::decode)?,
            linkage: decoder.field(2, LinkageClass::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedPersistentSymbolRequestTable {
    requests: Vec<DecodedPersistentSymbolRequest>,
}

impl DecodedPersistentSymbolRequestTable {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<PersistentSymbolRequestTable, PersistentSymbolResolutionError<E>>
    where
        R: PersistentSymbolResolver<E>,
    {
        for pair in self.requests.windows(2) {
            match compare_decoded_requests(&pair[0], &pair[1]) {
                Ordering::Less => {}
                Ordering::Equal => {
                    return Err(PersistentSymbolResolutionError::DuplicateRequest {
                        kind: pair[0].key.kind(),
                    });
                }
                Ordering::Greater => {
                    return Err(PersistentSymbolResolutionError::NonCanonicalRequestOrder {
                        previous: pair[0].key.kind(),
                        current: pair[1].key.kind(),
                    });
                }
            }
        }

        let mut requests = Vec::new();
        requests
            .try_reserve_exact(self.requests.len())
            .map_err(|_| PersistentSymbolResolutionError::Allocation)?;
        for request in self.requests {
            requests.push(request.resolve(resolver)?);
        }
        Ok(PersistentSymbolRequestTable { requests })
    }
}

impl WireEncode for DecodedPersistentSymbolRequestTable {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.requests.len() as u64)?;
        for request in &self.requests {
            request.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedPersistentSymbolRequestTable {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedPersistentSymbolRequest::decode(decoder))
            .map(|requests| Self { requests })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistentSymbolResolutionError<E> {
    Reference(E),
    Symbol(PersistentSymbolError),
    DuplicateRequest {
        kind: PersistentSymbolKind,
    },
    NonCanonicalRequestOrder {
        previous: PersistentSymbolKind,
        current: PersistentSymbolKind,
    },
    Allocation,
}

impl<E: fmt::Display> fmt::Display for PersistentSymbolResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Symbol(error) => error.fmt(formatter),
            Self::DuplicateRequest { kind } => {
                write!(
                    formatter,
                    "duplicate persistent symbol request for {}",
                    kind.symbol_tag()
                )
            }
            Self::NonCanonicalRequestOrder { previous, current } => write!(
                formatter,
                "persistent symbol request kind {} must precede {}",
                current.symbol_tag(),
                previous.symbol_tag()
            ),
            Self::Allocation => formatter.write_str("failed to allocate persistent symbol table"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for PersistentSymbolResolutionError<E> {}

fn compare_decoded_requests(
    left: &DecodedPersistentSymbolRequest,
    right: &DecodedPersistentSymbolRequest,
) -> Ordering {
    left.key
        .kind()
        .tag()
        .cmp(&right.key.kind().tag())
        .then_with(|| left.key.owner_bytes().cmp(right.key.owner_bytes()))
}

fn decode_id<I, T>(
    decoder: &mut Decoder<'_>,
    build: impl FnOnce(DecodedPersistentId<I>) -> T,
) -> Result<T, WireError>
where
    I: crate::PersistentId,
{
    decoder.field(1, DecodedPersistentId::decode).map(build)
}

fn unknown_tag(decoder: &Decoder<'_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

#[cfg(test)]
mod tests;
