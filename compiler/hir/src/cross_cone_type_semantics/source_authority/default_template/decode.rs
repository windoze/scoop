use scoop_identity::{DecodedSignatureTypeKey, StructuralDefinitionPath};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::DecodedDefaultSourceReferencesV1;
use crate::{
    CanonicalBooleanV1, DecodedCanonicalBinderUseListV1, DecodedCanonicalTemplateLocalTableV1,
    DecodedCanonicalTemplateValueParametersV1, DecodedExportDefaultBodyV1,
    DecodedExportDefinitionSourceV1, DecodedOptionalTemplateReceiverV1,
    DecodedPersistentLexicalRootV1, DecodedProtectedDefaultTemplateKeyV1,
};

mod resolve;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultSourceTemplateV1 {
    pub(super) key: DecodedProtectedDefaultTemplateKeyV1,
    pub(super) definition_root: DecodedPersistentLexicalRootV1,
    pub(super) definition_path: StructuralDefinitionPath,
    pub(super) locals: DecodedCanonicalTemplateLocalTableV1,
    pub(super) body: DecodedExportDefaultBodyV1,
    pub(super) result: DecodedSignatureTypeKey,
    pub(super) allows_suspend: CanonicalBooleanV1,
    pub(super) type_parameters: DecodedCanonicalBinderUseListV1,
    pub(super) receiver: DecodedOptionalTemplateReceiverV1,
    pub(super) value_parameters: DecodedCanonicalTemplateValueParametersV1,
    pub(super) references: DecodedDefaultSourceReferencesV1,
    pub(super) definition_origin: DecodedExportDefinitionSourceV1,
}
impl WireDecode for DecodedDefaultSourceTemplateV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(12)?;
        Ok(Self {
            key: decoder.field(1, DecodedProtectedDefaultTemplateKeyV1::decode)?,
            definition_root: decoder.field(2, DecodedPersistentLexicalRootV1::decode)?,
            definition_path: decoder.field(3, StructuralDefinitionPath::decode)?,
            locals: decoder.field(4, DecodedCanonicalTemplateLocalTableV1::decode)?,
            body: decoder.field(5, DecodedExportDefaultBodyV1::decode)?,
            result: decoder.field(6, DecodedSignatureTypeKey::decode)?,
            allows_suspend: decoder.field(7, CanonicalBooleanV1::decode)?,
            type_parameters: decoder.field(8, DecodedCanonicalBinderUseListV1::decode)?,
            receiver: decoder.field(9, DecodedOptionalTemplateReceiverV1::decode)?,
            value_parameters: decoder
                .field(10, DecodedCanonicalTemplateValueParametersV1::decode)?,
            references: decoder.field(11, DecodedDefaultSourceReferencesV1::decode)?,
            definition_origin: decoder.field(12, DecodedExportDefinitionSourceV1::decode)?,
        })
    }
}
impl WireEncode for DecodedDefaultSourceTemplateV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(12)?;
        encoder.field(1)?;
        self.key.encode(encoder)?;
        encoder.field(2)?;
        self.definition_root.encode(encoder)?;
        encoder.field(3)?;
        self.definition_path.encode(encoder)?;
        encoder.field(4)?;
        self.locals.encode(encoder)?;
        encoder.field(5)?;
        self.body.encode(encoder)?;
        encoder.field(6)?;
        self.result.encode(encoder)?;
        encoder.field(7)?;
        self.allows_suspend.encode(encoder)?;
        encoder.field(8)?;
        self.type_parameters.encode(encoder)?;
        encoder.field(9)?;
        self.receiver.encode(encoder)?;
        encoder.field(10)?;
        self.value_parameters.encode(encoder)?;
        encoder.field(11)?;
        self.references.encode(encoder)?;
        encoder.field(12)?;
        self.definition_origin.encode(encoder)
    }
}
