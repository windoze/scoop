use super::*;
use scoop_identity::{DecodedPersistentId, DecodedSignatureTypeKey};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode};

pub struct IndexedExportGenericDelegateTemplateV1<'a> {
    source: &'a ExportGenericDelegateTemplateV1,
    initializer: crate::IndexedExportGenericCallableBodyV1<'a>,
}

impl ExportGenericDelegateTemplateV1 {
    pub fn index_locals(
        &self,
    ) -> Result<IndexedExportGenericDelegateTemplateV1<'_>, crate::GenericCallableBodyIndexError>
    {
        Ok(IndexedExportGenericDelegateTemplateV1 {
            source: self,
            initializer: self.initializer.index_locals()?,
        })
    }
}

impl WireEncode for IndexedExportGenericDelegateTemplateV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.source.property.encode(encoder)?;
        encoder.field(2)?;
        self.source.effective_type.encode(encoder)?;
        encoder.field(3)?;
        self.initializer.encode(encoder)?;
        encoder.field(4)?;
        encoder.text(&self.source.diagnostic_path)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedExportGenericDelegateTemplateV1 {
    property: DecodedPersistentId<PersistentExtensionPropertyId>,
    effective_type: DecodedSignatureTypeKey,
    initializer: crate::DecodedExportGenericCallableBodyV1,
    diagnostic_path: String,
}

impl DecodedExportGenericDelegateTemplateV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExportGenericDelegateTemplateV1, GenericDelegateTemplateResolutionError<E>>
    where
        R: crate::DefaultStatementReferenceResolver<E>,
    {
        use GenericDelegateTemplateResolutionError as Error;
        let property = resolver.resolve(self.property).map_err(Error::Identity)?;
        let effective_type = self
            .effective_type
            .resolve(resolver)
            .map_err(Error::Identity)?;
        let initializer = self
            .initializer
            .resolve(resolver)
            .map_err(|error| Error::Body(Box::new(error)))?;
        ExportGenericDelegateTemplateV1::try_new(
            property,
            effective_type,
            initializer,
            self.diagnostic_path,
        )
        .map_err(Error::Record)
    }
}

impl WireEncode for DecodedExportGenericDelegateTemplateV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.property.encode(encoder)?;
        encoder.field(2)?;
        self.effective_type.encode(encoder)?;
        encoder.field(3)?;
        self.initializer.encode(encoder)?;
        encoder.field(4)?;
        encoder.text(&self.diagnostic_path)
    }
}

impl WireDecode for DecodedExportGenericDelegateTemplateV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            property: decoder.field(1, DecodedPersistentId::decode)?,
            effective_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            initializer: decoder.field(3, crate::DecodedExportGenericCallableBodyV1::decode)?,
            diagnostic_path: decoder.field(4, Decoder::owned_text)?,
        })
    }
}
