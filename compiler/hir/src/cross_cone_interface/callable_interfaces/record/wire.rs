use super::*;

impl WireEncode for CallableDeclarationRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(11)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.owner.encode(encoder)?;
        encoder.field(3)?;
        self.type_parameters.encode(encoder)?;
        encoder.field(4)?;
        OptionalSignatureType::from_option(self.receiver.clone()).encode(encoder)?;
        encoder.field(5)?;
        self.parameters.encode(encoder)?;
        encoder.field(6)?;
        self.result.encode(encoder)?;
        encoder.field(7)?;
        self.effects.encode(encoder)?;
        encoder.field(8)?;
        self.modality.encode(encoder)?;
        encoder.field(9)?;
        self.visibility.encode(encoder)?;
        encoder.field(10)?;
        self.slots.encode(encoder)?;
        encoder.field(11)?;
        encoder.array(self.context_parameters.len() as u64)?;
        for parameter in &self.context_parameters {
            parameter.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCallableDeclarationRecordV1 {
    declaration: DecodedCallableTemplateOrigin,
    owner: DecodedPublicDeclarationOwnerV1,
    type_parameters: DecodedCanonicalBinderListV1,
    receiver: DecodedOptionalSignatureType,
    parameters: DecodedCanonicalSourceParameterShapesV1,
    result: DecodedSignatureTypeKey,
    effects: DecodedCallableSourceEffectsV1,
    modality: CallableModalityV1,
    visibility: DeclaredVisibilityV1,
    slots: DecodedCanonicalPersistentIdsV1<PersistentDispatchSlotId>,
    context_parameters: Vec<crate::DecodedSourceParameterShapeV1>,
}

impl DecodedCallableDeclarationRecordV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CallableDeclarationRecordV1, CallableInterfaceRecordResolutionError<E>>
    where
        R: CallableInterfaceRecordResolver<E>,
    {
        let declaration = self
            .declaration
            .resolve(resolver)
            .map_err(CallableInterfaceRecordResolutionError::Declaration)?;
        let owner = self
            .owner
            .resolve(resolver)
            .map_err(CallableInterfaceRecordResolutionError::Owner)?;
        let type_parameters = self
            .type_parameters
            .resolve(resolver)
            .map_err(CallableInterfaceRecordResolutionError::TypeParameters)?;
        let receiver = self
            .receiver
            .resolve(resolver)
            .map_err(CallableInterfaceRecordResolutionError::Receiver)?;
        let receiver = match receiver {
            OptionalSignatureType::Absent => None,
            OptionalSignatureType::Present(receiver) => Some(*receiver),
        };
        let parameters = self
            .parameters
            .resolve(resolver)
            .map_err(CallableInterfaceRecordResolutionError::Parameters)?;
        let result = self
            .result
            .resolve(resolver)
            .map_err(CallableInterfaceRecordResolutionError::Result)?;
        let effects = self
            .effects
            .validate()
            .map_err(CallableInterfaceRecordResolutionError::Effects)?;
        CallableDeclarationRecordV1::try_new(
            declaration,
            owner,
            type_parameters,
            receiver,
            parameters,
            result,
            effects,
            self.modality,
            self.visibility,
            self.slots
                .resolve(resolver)
                .map_err(CallableInterfaceRecordResolutionError::Slots)?,
            self.context_parameters
                .into_iter()
                .enumerate()
                .map(|(index, parameter)| {
                    parameter.resolve(resolver).map_err(|error| {
                        CallableInterfaceRecordResolutionError::ContextParameter { index, error }
                    })
                })
                .collect::<Result<_, _>>()?,
        )
        .map_err(CallableInterfaceRecordResolutionError::Record)
    }
}

impl WireEncode for DecodedCallableDeclarationRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(11)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.owner.encode(encoder)?;
        encoder.field(3)?;
        self.type_parameters.encode(encoder)?;
        encoder.field(4)?;
        self.receiver.encode(encoder)?;
        encoder.field(5)?;
        self.parameters.encode(encoder)?;
        encoder.field(6)?;
        self.result.encode(encoder)?;
        encoder.field(7)?;
        self.effects.encode(encoder)?;
        encoder.field(8)?;
        self.modality.encode(encoder)?;
        encoder.field(9)?;
        self.visibility.encode(encoder)?;
        encoder.field(10)?;
        self.slots.encode(encoder)?;
        encoder.field(11)?;
        encoder.array(self.context_parameters.len() as u64)?;
        for parameter in &self.context_parameters {
            parameter.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCallableDeclarationRecordV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(11)?;
        let value = Self {
            declaration: decoder.field(1, DecodedCallableTemplateOrigin::decode)?,
            owner: decoder.field(2, DecodedPublicDeclarationOwnerV1::decode)?,
            type_parameters: decoder.field(3, DecodedCanonicalBinderListV1::decode)?,
            receiver: decoder.field(4, DecodedOptionalSignatureType::decode)?,
            parameters: decoder.field(5, DecodedCanonicalSourceParameterShapesV1::decode)?,
            result: decoder.field(6, DecodedSignatureTypeKey::decode)?,
            effects: decoder.field(7, DecodedCallableSourceEffectsV1::decode)?,
            modality: decoder.field(8, CallableModalityV1::decode)?,
            visibility: decoder.field(9, DeclaredVisibilityV1::decode)?,
            slots: decoder.field(10, DecodedCanonicalPersistentIdsV1::decode)?,
            context_parameters: decoder.field(11, |decoder| {
                decoder.decode_array(|decoder, _| {
                    crate::DecodedSourceParameterShapeV1::decode(decoder)
                })
            })?,
        };

        Ok(value)
    }
}
