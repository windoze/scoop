use super::*;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalExportGenericDelegatesV1 {
    records: Vec<ExportGenericDelegateTemplateV1>,
}

impl CanonicalExportGenericDelegatesV1 {
    pub fn try_new(
        mut records: Vec<ExportGenericDelegateTemplateV1>,
    ) -> Result<Self, GenericDelegateTemplateBuildError> {
        records.sort_unstable_by_key(ExportGenericDelegateTemplateV1::property);
        require_order(&records)?;
        Ok(Self { records })
    }

    pub fn records(&self) -> &[ExportGenericDelegateTemplateV1] {
        &self.records
    }

    pub fn get(
        &self,
        property: PersistentExtensionPropertyId,
    ) -> Option<&ExportGenericDelegateTemplateV1> {
        self.records
            .binary_search_by_key(&property, ExportGenericDelegateTemplateV1::property)
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn index_locals(
        &self,
    ) -> Result<IndexedExportGenericDelegatesV1<'_>, crate::GenericCallableBodyIndexError> {
        let records = self
            .records
            .iter()
            .map(ExportGenericDelegateTemplateV1::index_locals)
            .collect::<Result<_, _>>()?;
        Ok(IndexedExportGenericDelegatesV1 { records })
    }
}

pub struct IndexedExportGenericDelegatesV1<'a> {
    records: Vec<IndexedExportGenericDelegateTemplateV1<'a>>,
}

impl WireEncode for IndexedExportGenericDelegatesV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalExportGenericDelegatesV1 {
    records: Vec<DecodedExportGenericDelegateTemplateV1>,
}

impl DecodedCanonicalExportGenericDelegatesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalExportGenericDelegatesV1, GenericDelegateTemplateResolutionError<E>>
    where
        R: crate::DefaultStatementReferenceResolver<E>,
    {
        let records = self
            .records
            .into_iter()
            .map(|record| record.resolve(resolver))
            .collect::<Result<Vec<_>, _>>()?;
        require_order(&records).map_err(GenericDelegateTemplateResolutionError::Record)?;
        Ok(CanonicalExportGenericDelegatesV1 { records })
    }
}

impl WireEncode for DecodedCanonicalExportGenericDelegatesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalExportGenericDelegatesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedExportGenericDelegateTemplateV1::decode(decoder))
            .map(|records| Self { records })
    }
}

fn require_order(
    records: &[ExportGenericDelegateTemplateV1],
) -> Result<(), GenericDelegateTemplateBuildError> {
    for (index, pair) in records.windows(2).enumerate() {
        if pair[0].property() >= pair[1].property() {
            return Err(GenericDelegateTemplateBuildError::RecordOrder {
                index: index + 1,
                property: pair[1].property(),
            });
        }
    }
    Ok(())
}
