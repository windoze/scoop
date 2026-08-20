use super::*;

impl WireEncode for PropertyDeclarationRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.owner.encode(encoder)?;
        encoder.field(3)?;
        self.type_parameters.encode(encoder)?;
        encoder.field(4)?;
        OptionalSignatureType::from_option(self.receiver.clone()).encode(encoder)?;
        encoder.field(5)?;
        self.value_type.encode(encoder)?;
        encoder.field(6)?;
        self.accessors.encode(encoder)?;
        encoder.field(7)?;
        self.representation.encode(encoder)?;
        encoder.field(8)?;
        self.visibility.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedPropertyDeclarationRecordV1 {
    declaration: DecodedPropertyDeclarationId,
    owner: DecodedPublicDeclarationOwnerV1,
    type_parameters: DecodedCanonicalBinderListV1,
    receiver: DecodedOptionalSignatureType,
    value_type: DecodedSignatureTypeKey,
    accessors: DecodedPropertyAccessorsV1,
    representation: PropertyRepresentationV1,
    visibility: DeclaredVisibilityV1,
}

impl DecodedPropertyDeclarationRecordV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<PropertyDeclarationRecordV1, PropertyInterfaceRecordResolutionError<E>>
    where
        R: PropertyInterfaceRecordResolver<E>,
    {
        let declaration = self
            .declaration
            .resolve(resolver)
            .map_err(PropertyInterfaceRecordResolutionError::Declaration)?;
        let owner = self
            .owner
            .resolve(resolver)
            .map_err(PropertyInterfaceRecordResolutionError::Owner)?;
        let type_parameters = self
            .type_parameters
            .resolve(resolver)
            .map_err(PropertyInterfaceRecordResolutionError::TypeParameters)?;
        let receiver = self
            .receiver
            .resolve(resolver)
            .map_err(PropertyInterfaceRecordResolutionError::Receiver)?;
        let receiver = match receiver {
            OptionalSignatureType::Absent => None,
            OptionalSignatureType::Present(receiver) => Some(*receiver),
        };
        let value_type = self
            .value_type
            .resolve(resolver)
            .map_err(PropertyInterfaceRecordResolutionError::ValueType)?;
        let accessors = self
            .accessors
            .resolve(resolver)
            .map_err(PropertyInterfaceRecordResolutionError::Capability)?;
        PropertyDeclarationRecordV1::try_new(
            declaration,
            owner,
            type_parameters,
            receiver,
            value_type,
            accessors,
            self.representation,
            self.visibility,
        )
        .map_err(PropertyInterfaceRecordResolutionError::Record)
    }
}

impl WireEncode for DecodedPropertyDeclarationRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.owner.encode(encoder)?;
        encoder.field(3)?;
        self.type_parameters.encode(encoder)?;
        encoder.field(4)?;
        self.receiver.encode(encoder)?;
        encoder.field(5)?;
        self.value_type.encode(encoder)?;
        encoder.field(6)?;
        self.accessors.encode(encoder)?;
        encoder.field(7)?;
        self.representation.encode(encoder)?;
        encoder.field(8)?;
        self.visibility.encode(encoder)
    }
}

impl WireDecode for DecodedPropertyDeclarationRecordV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(8)?;
        Ok(Self {
            declaration: decoder.field(1, DecodedPropertyDeclarationId::decode)?,
            owner: decoder.field(2, DecodedPublicDeclarationOwnerV1::decode)?,
            type_parameters: decoder.field(3, DecodedCanonicalBinderListV1::decode)?,
            receiver: decoder.field(4, DecodedOptionalSignatureType::decode)?,
            value_type: decoder.field(5, DecodedSignatureTypeKey::decode)?,
            accessors: decoder.field(6, DecodedPropertyAccessorsV1::decode)?,
            representation: decoder.field(7, PropertyRepresentationV1::decode)?,
            visibility: decoder.field(8, DeclaredVisibilityV1::decode)?,
        })
    }
}
