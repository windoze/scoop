use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropertyInterfaceRecordV1 {
    data: PropertyDeclarationRecordV1,
    access: PropertyPublicAccessV1,
    setter_access: PropertySetterPublicAccessV1,
}

impl PropertyInterfaceRecordV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        declaration: PropertyDeclarationId,
        owner: PublicDeclarationOwnerV1,
        type_parameters: CanonicalBinderListV1,
        receiver: Option<SignatureTypeKey>,
        value_type: SignatureTypeKey,
        accessors: PropertyAccessorsV1,
        representation: PropertyRepresentationV1,
        access: PropertyPublicAccessV1,
        setter_access: PropertySetterPublicAccessV1,
    ) -> Result<Self, PropertyInterfaceRecordBuildError> {
        validate_declaration_shape(declaration, owner, &type_parameters, receiver.is_some())?;
        validate_access_shape(declaration, owner, access)?;
        let data = PropertyDeclarationRecordV1::try_new(
            declaration,
            owner,
            type_parameters,
            receiver,
            value_type,
            accessors,
            representation,
            DeclaredVisibilityV1::Public,
        )?;
        Self::from_declaration(data, access, setter_access)
    }

    pub fn from_declaration(
        data: PropertyDeclarationRecordV1,
        access: PropertyPublicAccessV1,
        setter_access: PropertySetterPublicAccessV1,
    ) -> Result<Self, PropertyInterfaceRecordBuildError> {
        if data.declared_visibility() != DeclaredVisibilityV1::Public {
            return Err(PropertyInterfaceRecordBuildError::NonPublicDeclaration(
                data.declared_visibility(),
            ));
        }
        if data.accessors().is_read_only()
            && setter_access != PropertySetterPublicAccessV1::Restricted
        {
            return Err(PropertyInterfaceRecordBuildError::MissingSetterLookup(
                data.declaration(),
            ));
        }
        validate_access_shape(data.declaration(), data.owner(), access)?;
        match data.representation() {
            PropertyRepresentationV1::Const if access != PropertyPublicAccessV1::DirectOnly => {
                return Err(PropertyInterfaceRecordBuildError::ConstMustBeDirect(
                    data.declaration(),
                ));
            }
            PropertyRepresentationV1::AbstractSlot
                if access != PropertyPublicAccessV1::PublicSlot =>
            {
                return Err(
                    PropertyInterfaceRecordBuildError::AbstractSlotAccessRequired(
                        data.declaration(),
                    ),
                );
            }
            PropertyRepresentationV1::Const
            | PropertyRepresentationV1::RuntimeAccessor
            | PropertyRepresentationV1::AbstractSlot => Ok(()),
        }?;
        Ok(Self {
            data,
            access,
            setter_access,
        })
    }

    pub const fn declaration_data(&self) -> &PropertyDeclarationRecordV1 {
        &self.data
    }
    pub const fn declaration(&self) -> PropertyDeclarationId {
        self.data.declaration()
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
    pub const fn value_type(&self) -> &SignatureTypeKey {
        self.data.value_type()
    }
    pub const fn capability(&self) -> PropertyCapabilityV1 {
        PropertyCapabilityV1::from_accessors(self.data.accessors(), self.setter_access)
    }
    pub const fn representation(&self) -> PropertyRepresentationV1 {
        self.data.representation()
    }
    pub const fn access(&self) -> PropertyPublicAccessV1 {
        self.access
    }
}

impl std::ops::Deref for PropertyInterfaceRecordV1 {
    type Target = PropertyDeclarationRecordV1;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl WireEncode for PropertyInterfaceRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.data.encode(encoder)?;
        encoder.field(2)?;
        self.access.encode(encoder)?;
        encoder.field(3)?;
        self.setter_access.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedPropertyInterfaceRecordV1 {
    data: DecodedPropertyDeclarationRecordV1,
    pub(super) access: PropertyPublicAccessV1,
    setter_access: PropertySetterPublicAccessV1,
}

impl DecodedPropertyInterfaceRecordV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<PropertyInterfaceRecordV1, PropertyInterfaceRecordResolutionError<E>>
    where
        R: PropertyInterfaceRecordResolver<E>,
    {
        PropertyInterfaceRecordV1::from_declaration(
            self.data.resolve(resolver)?,
            self.access,
            self.setter_access,
        )
        .map_err(PropertyInterfaceRecordResolutionError::Record)
    }
}

impl WireDecode for DecodedPropertyInterfaceRecordV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            data: decoder.field(1, DecodedPropertyDeclarationRecordV1::decode)?,
            access: decoder.field(2, PropertyPublicAccessV1::decode)?,
            setter_access: decoder.field(3, PropertySetterPublicAccessV1::decode)?,
        })
    }
}

impl WireEncode for DecodedPropertyInterfaceRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.data.encode(encoder)?;
        encoder.field(2)?;
        self.access.encode(encoder)?;
        encoder.field(3)?;
        self.setter_access.encode(encoder)
    }
}
