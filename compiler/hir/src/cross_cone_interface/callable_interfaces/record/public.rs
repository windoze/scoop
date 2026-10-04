use super::*;
use std::ops::Deref;

/// A public lookup relation over the sole source declaration record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableInterfaceRecordV1 {
    data: CallableDeclarationRecordV1,
    access: PublicLookupAccessV1,
}

impl CallableInterfaceRecordV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        declaration: CallableTemplateOrigin,
        owner: PublicDeclarationOwnerV1,
        type_parameters: CanonicalBinderListV1,
        receiver: Option<SignatureTypeKey>,
        parameters: CanonicalSourceParameterShapesV1,
        result: SignatureTypeKey,
        effects: CallableSourceEffectsV1,
        modality: CallableModalityV1,
        access: PublicLookupAccessV1,
        slots: CanonicalPersistentIdsV1<PersistentDispatchSlotId>,
        context_parameters: Vec<crate::SourceParameterShapeV1>,
    ) -> Result<Self, CallableInterfaceRecordBuildError> {
        validate_type_parameter_shape(declaration, &type_parameters)?;
        validate_receiver_shape(owner, receiver.is_some())?;
        validate_owner_shape(declaration, owner)?;
        validate_dispatch_shape(declaration, owner, &effects, modality, access)?;
        let data = CallableDeclarationRecordV1::try_new(
            declaration,
            owner,
            type_parameters,
            receiver,
            parameters,
            result,
            effects,
            modality,
            DeclaredVisibilityV1::Public,
            slots,
            context_parameters,
        )?;
        Self::from_declaration(data, access)
    }

    pub fn from_declaration(
        data: CallableDeclarationRecordV1,
        access: PublicLookupAccessV1,
    ) -> Result<Self, CallableInterfaceRecordBuildError> {
        if data.declared_visibility() != DeclaredVisibilityV1::Public {
            return Err(CallableInterfaceRecordBuildError::NonPublicDeclaration(
                data.declaration(),
            ));
        }
        validate_dispatch_shape(
            data.declaration(),
            data.owner(),
            &data.effects(),
            data.modality(),
            access,
        )?;
        if !data.slots.is_empty() && access != PublicLookupAccessV1::PublicSlot {
            return Err(CallableInterfaceRecordBuildError::InvalidSlotRelations {
                declaration: data.declaration(),
            });
        }
        Ok(Self { data, access })
    }

    pub const fn owner(&self) -> PublicDeclarationOwnerV1 {
        self.data.owner()
    }
    pub const fn type_parameters(&self) -> &CanonicalBinderListV1 {
        self.data.type_parameters()
    }
    pub fn receiver(&self) -> Option<&SignatureTypeKey> {
        self.data.receiver()
    }
    pub const fn parameters(&self) -> &CanonicalSourceParameterShapesV1 {
        self.data.parameters()
    }
    pub const fn result(&self) -> &SignatureTypeKey {
        self.data.result()
    }
    pub fn effects(&self) -> CallableSourceEffectsV1 {
        self.data.effects()
    }
    pub const fn modality(&self) -> CallableModalityV1 {
        self.data.modality()
    }
    pub const fn access(&self) -> PublicLookupAccessV1 {
        self.access
    }
    pub const fn declaration_data(&self) -> &CallableDeclarationRecordV1 {
        &self.data
    }
    pub const fn declaration(&self) -> CallableTemplateOrigin {
        self.data.declaration()
    }
}

impl Deref for CallableInterfaceRecordV1 {
    type Target = CallableDeclarationRecordV1;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl WireEncode for CallableInterfaceRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.data.encode(encoder)?;
        encoder.field(2)?;
        self.access.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCallableInterfaceRecordV1 {
    data: DecodedCallableDeclarationRecordV1,
    access: PublicLookupAccessV1,
}

impl DecodedCallableInterfaceRecordV1 {
    pub fn resolve<R: CallableInterfaceRecordResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<CallableInterfaceRecordV1, CallableInterfaceRecordResolutionError<E>> {
        CallableInterfaceRecordV1::from_declaration(self.data.resolve(resolver)?, self.access)
            .map_err(CallableInterfaceRecordResolutionError::Record)
    }
}
impl WireEncode for DecodedCallableInterfaceRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.data.encode(encoder)?;
        encoder.field(2)?;
        self.access.encode(encoder)
    }
}
impl WireDecode for DecodedCallableInterfaceRecordV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            data: decoder.field(1, DecodedCallableDeclarationRecordV1::decode)?,
            access: decoder.field(2, PublicLookupAccessV1::decode)?,
        })
    }
}
