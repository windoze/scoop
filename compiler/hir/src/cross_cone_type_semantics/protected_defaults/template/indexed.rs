use scoop_identity::LocalValueSelector;
use scoop_wire::{Encoder, WireEncode};

use super::*;
use crate::{
    ExportDefaultTemplateIndexError, IndexedCanonicalTemplateValueParametersV1,
    IndexedExportDefaultBodyV1, IndexedOptionalTemplateReceiverV1, TemplateLocalIndexResolver,
    TemplateLocalLookupError,
};

pub type ProtectedDefaultTemplateIndexError = ExportDefaultTemplateIndexError;

struct LocalIndex<'a>(&'a CanonicalTemplateLocalTableV1);
impl TemplateLocalIndexResolver for LocalIndex<'_> {
    type Error = TemplateLocalLookupError;
    fn resolve_template_local_index(
        &mut self,
        selector: &LocalValueSelector,
    ) -> Result<u32, Self::Error> {
        self.0
            .index_of(selector)
            .ok_or_else(|| TemplateLocalLookupError::MissingSelector(selector.clone()))
    }
}

impl ProtectedDefaultTemplateV1 {
    pub fn index_locals(
        &self,
    ) -> Result<IndexedProtectedDefaultTemplateV1<'_>, ProtectedDefaultTemplateIndexError> {
        let mut locals = LocalIndex(&self.locals);
        let body = self
            .body
            .index_locals(&mut locals)
            .map_err(ExportDefaultTemplateIndexError::Body)?;
        let receiver = self
            .receiver
            .index_local(&mut locals)
            .map_err(ExportDefaultTemplateIndexError::Receiver)?;
        let value_parameters = self
            .value_parameters
            .index_locals(&mut locals)
            .map_err(ExportDefaultTemplateIndexError::ValueParameters)?;
        Ok(IndexedProtectedDefaultTemplateV1 {
            template: self,
            body,
            receiver,
            value_parameters,
        })
    }
}

#[derive(Debug)]
pub struct IndexedProtectedDefaultTemplateV1<'a> {
    template: &'a ProtectedDefaultTemplateV1,
    body: IndexedExportDefaultBodyV1<'a>,
    receiver: IndexedOptionalTemplateReceiverV1<'a>,
    value_parameters: IndexedCanonicalTemplateValueParametersV1<'a>,
}
impl WireEncode for IndexedProtectedDefaultTemplateV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(12)?;
        encoder.field(1)?;
        self.template.key.encode(encoder)?;
        encoder.field(2)?;
        self.template.definition_root.encode(encoder)?;
        encoder.field(3)?;
        self.template.definition_path.encode(encoder)?;
        encoder.field(4)?;
        self.template.locals.encode(encoder)?;
        encoder.field(5)?;
        self.body.encode(encoder)?;
        encoder.field(6)?;
        self.template.result.encode(encoder)?;
        encoder.field(7)?;
        self.template.allows_suspend.encode(encoder)?;
        encoder.field(8)?;
        self.template.type_parameters.encode(encoder)?;
        encoder.field(9)?;
        self.receiver.encode(encoder)?;
        encoder.field(10)?;
        self.value_parameters.encode(encoder)?;
        encoder.field(11)?;
        self.template.references.encode(encoder)?;
        encoder.field(12)?;
        self.template.definition_origin.encode(encoder)
    }
}
