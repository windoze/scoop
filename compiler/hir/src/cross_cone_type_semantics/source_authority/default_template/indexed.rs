use scoop_wire::{Encoder, WireEncode};

use super::*;
use crate::{
    IndexedCanonicalTemplateValueParametersV1, IndexedExportDefaultBodyV1,
    IndexedOptionalTemplateReceiverV1,
};

impl DefaultSourceTemplateV1 {
    pub fn index_locals(
        &self,
        meter: &mut scoop_wire::BudgetMeter,
    ) -> Result<IndexedDefaultSourceTemplateV1<'_>, DefaultSourceTemplateIndexError> {
        resources::preflight(&self.body, &self.value_parameters, meter)
            .map_err(DefaultSourceTemplateIndexError::Resource)?;
        let mut locals = resources::LocalIndex::new(&self.locals, meter);
        let body = self
            .body
            .index_locals(&mut locals)
            .map_err(DefaultSourceTemplateIndexError::Body)?;
        let receiver = self
            .receiver
            .index_local(&mut locals)
            .map_err(DefaultSourceTemplateIndexError::Receiver)?;
        let value_parameters = self
            .value_parameters
            .index_locals(&mut locals)
            .map_err(DefaultSourceTemplateIndexError::ValueParameters)?;
        Ok(IndexedDefaultSourceTemplateV1 {
            template: self,
            body,
            receiver,
            value_parameters,
        })
    }
}

#[derive(Debug)]
pub struct IndexedDefaultSourceTemplateV1<'a> {
    template: &'a DefaultSourceTemplateV1,
    body: IndexedExportDefaultBodyV1<'a>,
    receiver: IndexedOptionalTemplateReceiverV1<'a>,
    value_parameters: IndexedCanonicalTemplateValueParametersV1<'a>,
}
impl WireEncode for IndexedDefaultSourceTemplateV1<'_> {
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
