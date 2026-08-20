use super::*;
use crate::{
    DecodedExportDefinitionSourceV1, DecodedExportTemplateFragmentV1,
    DefaultStatementReferenceResolver, IndexedExportTemplateFragmentV1, TemplateFragmentIndexError,
};

pub(super) struct IndexedReleaseTemplate<'a> {
    origin: &'a ExportDefinitionSourceV1,
    body: IndexedExportTemplateFragmentV1<'a>,
}

impl<'a> IndexedReleaseTemplate<'a> {
    pub(super) fn new(
        source: &'a ExportReleaseTemplateV1,
    ) -> Result<Self, TemplateFragmentIndexError> {
        Ok(Self {
            origin: source.definition_origin(),
            body: source.body().index_locals()?,
        })
    }
}

impl WireEncode for IndexedReleaseTemplate<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.origin.encode(encoder)?;
        encoder.field(2)?;
        self.body.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecodedReleaseTemplate {
    origin: DecodedExportDefinitionSourceV1,
    body: DecodedExportTemplateFragmentV1,
}

impl DecodedReleaseTemplate {
    pub(super) fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExportReleaseTemplateV1, GenericInitializationResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E>,
    {
        use GenericInitializationResolutionError as Error;
        let origin = self.origin.resolve(resolver).map_err(Error::Origin)?;
        let body = self
            .body
            .resolve(resolver)
            .map_err(|error| Error::Fragment(Box::new(error)))?;
        ExportReleaseTemplateV1::try_new(origin, body).map_err(Error::Record)
    }
}

impl WireDecode for DecodedReleaseTemplate {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            origin: decoder.field(1, DecodedExportDefinitionSourceV1::decode)?,
            body: decoder.field(2, DecodedExportTemplateFragmentV1::decode)?,
        })
    }
}

impl WireEncode for DecodedReleaseTemplate {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.origin.encode(encoder)?;
        encoder.field(2)?;
        self.body.encode(encoder)
    }
}
