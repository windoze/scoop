use super::*;
use scoop_identity::SignatureTypeKey;
use scoop_wire::{Encoder, WireEncode, WirePath};

pub struct IndexedProtectedCallableSourceInterfaceV1<'a> {
    owner: CallableTemplateOrigin,
    parameters: Vec<IndexedProtectedSourceParameterV1<'a>>,
}
impl ProtectedCallableSourceInterfaceV1 {
    pub fn index_templates<'a>(
        &'a self,
        keys: &ProtectedDefaultKeyIndexV1,
    ) -> Result<IndexedProtectedCallableSourceInterfaceV1<'a>, ProtectedSourceIndexError> {
        let mut parameters = Vec::new();
        scoop_wire::allocation::try_reserve(
            &mut parameters,
            self.parameters.parameters().len(),
            &WirePath::root(),
        )
        .map_err(ProtectedSourceIndexError::Resource)?;
        for parameter in self.parameters.parameters() {
            let calling = match parameter.calling() {
                ProtectedParameterCallingV1::Required => IndexedCallingV1::Required,
                ProtectedParameterCallingV1::Default { template } => IndexedCallingV1::Default(
                    keys.index(*template)
                        .map_err(ProtectedSourceIndexError::Build)?,
                ),
                ProtectedParameterCallingV1::VarargEmpty { element_type } => {
                    IndexedCallingV1::VarargEmpty(element_type)
                }
                ProtectedParameterCallingV1::VarargDefault {
                    element_type,
                    template,
                } => IndexedCallingV1::VarargDefault(
                    element_type,
                    keys.index(*template)
                        .map_err(ProtectedSourceIndexError::Build)?,
                ),
            };
            parameters.push(IndexedProtectedSourceParameterV1 { parameter, calling });
        }
        Ok(IndexedProtectedCallableSourceInterfaceV1 {
            owner: self.owner,
            parameters,
        })
    }
}
impl WireEncode for IndexedProtectedCallableSourceInterfaceV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        wire::sequence(encoder, &self.parameters)
    }
}
struct IndexedProtectedSourceParameterV1<'a> {
    parameter: &'a ProtectedSourceParameterV1,
    calling: IndexedCallingV1<'a>,
}
enum IndexedCallingV1<'a> {
    Required,
    Default(ProtectedDefaultTemplateIndexV1),
    VarargEmpty(&'a SignatureTypeKey),
    VarargDefault(&'a SignatureTypeKey, ProtectedDefaultTemplateIndexV1),
}
impl WireEncode for IndexedProtectedSourceParameterV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.parameter.name().encode(encoder)?;
        encoder.field(2)?;
        self.parameter.value_type().encode(encoder)?;
        encoder.field(3)?;
        self.calling.encode(encoder)?;
        encoder.field(4)?;
        self.parameter.definition_origin().encode(encoder)
    }
}
impl WireEncode for IndexedCallingV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Required => wire::tag(encoder, 1, 1),
            Self::Default(index) => {
                wire::tag(encoder, 2, 2)?;
                encoder.field(1)?;
                index.encode(encoder)
            }
            Self::VarargEmpty(ty) => {
                wire::tag(encoder, 2, 3)?;
                encoder.field(1)?;
                ty.encode(encoder)
            }
            Self::VarargDefault(ty, index) => {
                wire::tag(encoder, 3, 4)?;
                encoder.field(1)?;
                ty.encode(encoder)?;
                encoder.field(2)?;
                index.encode(encoder)
            }
        }
    }
}
