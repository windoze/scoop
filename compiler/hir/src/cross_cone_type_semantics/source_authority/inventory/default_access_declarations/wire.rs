use super::*;
use crate::{
    DeclarationAccessSourceResolutionError, DecodedDeclarationAccessSourceV1,
    SourceNominalIdResolver,
};
use scoop_identity::{
    ConeIdentity, DecodedDefinitionOriginSubject, DefinitionOriginSubjectResolver,
    PersistentIdResolver, PersistentKeyResolver, PersistentSourceContextId, SourceContextKey,
};
use scoop_wire::{Decoder, WireDecode, WireErrorKind};
mod subject;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultSourceAccessDeclarationV1 {
    subject: DecodedDefinitionOriginSubject,
    access: DecodedDeclarationAccessSourceV1,
}
impl WireDecode for DecodedDefaultSourceAccessDeclarationV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            subject: decoder.field(1, subject::decode)?,
            access: decoder.field(2, DecodedDeclarationAccessSourceV1::decode)?,
        })
    }
}
impl WireEncode for DecodedDefaultSourceAccessDeclarationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.subject.encode(encoder)?;
        encoder.field(2)?;
        self.access.encode(encoder)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalDefaultSourceAccessDeclarationsV1 {
    records: Vec<DecodedDefaultSourceAccessDeclarationV1>,
}
impl WireDecode for DecodedCanonicalDefaultSourceAccessDeclarationsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedDefaultSourceAccessDeclarationV1::decode(d))
            .map(|records| Self { records })
    }
}
impl WireEncode for DecodedCanonicalDefaultSourceAccessDeclarationsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::super::wire::sequence(encoder, &self.records)
    }
}
impl DecodedCanonicalDefaultSourceAccessDeclarationsV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<
        CanonicalDefaultSourceAccessDeclarationsV1,
        DefaultSourceAccessDeclarationResolutionError<E>,
    >
    where
        R: DefinitionOriginSubjectResolver<E>
            + SourceNominalIdResolver<E>
            + PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>,
    {
        use DefaultSourceAccessDeclarationResolutionError as Error;
        let mut records = reserve(self.records.len()).map_err(Error::Inventory)?;
        for record in self.records.into_iter() {
            let subject = record.subject.resolve(resolver).map_err(Error::Identity)?;
            let access = record.access.resolve(resolver).map_err(Error::Access)?;
            records.push(
                DefaultSourceAccessDeclarationV1::try_new(subject, access)
                    .map_err(Error::Inventory)?,
            );
        }
        CanonicalDefaultSourceAccessDeclarationsV1::from_ordered(records).map_err(Error::Inventory)
    }
}
#[derive(Debug)]
pub enum DefaultSourceAccessDeclarationResolutionError<E> {
    Resource(WireError),
    Inventory(SourceInventoryError),
    Identity(E),
    Access(DeclarationAccessSourceResolutionError<E>),
}
impl<E: fmt::Display> fmt::Display for DefaultSourceAccessDeclarationResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Inventory(e) => e.fmt(f),
            Self::Identity(e) => e.fmt(f),
            Self::Access(e) => e.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for DefaultSourceAccessDeclarationResolutionError<E>
{
}
