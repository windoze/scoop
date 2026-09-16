use scoop_identity::{
    DecodedOptionalSignatureType, DecodedSignatureTypeKey, OptionalSignatureType,
    PersistentIdResolver, PersistentPropertyAccessorId, PropertyOwner, SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    DecodedPropertyCapabilityV1, PropertyCapabilityV1, PropertyPublicAccessV1,
    PropertyRepresentationV1,
};
use crate::{
    CanonicalBinderListV1, DecodedCanonicalBinderListV1, DecodedPropertyDeclarationId,
    DecodedPublicDeclarationOwnerV1, PropertyDeclarationId, PropertyDeclarationIdResolver,
    PublicDeclarationOwnerV1, SignatureTypeReferenceResolver,
};

mod errors;

pub use errors::{PropertyInterfaceRecordBuildError, PropertyInterfaceRecordResolutionError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropertyInterfaceRecordV1 {
    declaration: PropertyDeclarationId,
    owner: PublicDeclarationOwnerV1,
    type_parameters: CanonicalBinderListV1,
    receiver: Option<SignatureTypeKey>,
    value_type: SignatureTypeKey,
    capability: PropertyCapabilityV1,
    representation: PropertyRepresentationV1,
    access: PropertyPublicAccessV1,
}

impl PropertyInterfaceRecordV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        declaration: PropertyDeclarationId,
        owner: PublicDeclarationOwnerV1,
        type_parameters: CanonicalBinderListV1,
        receiver: Option<SignatureTypeKey>,
        value_type: SignatureTypeKey,
        capability: PropertyCapabilityV1,
        representation: PropertyRepresentationV1,
        access: PropertyPublicAccessV1,
    ) -> Result<Self, PropertyInterfaceRecordBuildError> {
        validate_declaration_shape(declaration, owner, &type_parameters, receiver.is_some())?;
        validate_access_shape(declaration, owner, access)?;
        validate_representation_shape(
            declaration,
            owner,
            &type_parameters,
            receiver.is_some(),
            capability,
            representation,
            access,
        )?;
        Ok(Self {
            declaration,
            owner,
            type_parameters,
            receiver,
            value_type,
            capability,
            representation,
            access,
        })
    }

    pub const fn declaration(&self) -> PropertyDeclarationId {
        self.declaration
    }

    pub const fn owner(&self) -> PublicDeclarationOwnerV1 {
        self.owner
    }

    pub const fn type_parameters(&self) -> &CanonicalBinderListV1 {
        &self.type_parameters
    }

    pub fn receiver(&self) -> Option<&SignatureTypeKey> {
        self.receiver.as_ref()
    }

    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }

    pub const fn capability(&self) -> PropertyCapabilityV1 {
        self.capability
    }

    pub const fn representation(&self) -> PropertyRepresentationV1 {
        self.representation
    }

    pub const fn access(&self) -> PropertyPublicAccessV1 {
        self.access
    }
}

impl WireEncode for PropertyInterfaceRecordV1 {
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
        self.capability.encode(encoder)?;
        encoder.field(7)?;
        self.representation.encode(encoder)?;
        encoder.field(8)?;
        self.access.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedPropertyInterfaceRecordV1 {
    declaration: DecodedPropertyDeclarationId,
    owner: DecodedPublicDeclarationOwnerV1,
    type_parameters: DecodedCanonicalBinderListV1,
    receiver: DecodedOptionalSignatureType,
    value_type: DecodedSignatureTypeKey,
    capability: DecodedPropertyCapabilityV1,
    representation: PropertyRepresentationV1,
    access: PropertyPublicAccessV1,
}

impl DecodedPropertyInterfaceRecordV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<PropertyInterfaceRecordV1, PropertyInterfaceRecordResolutionError<E>>
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
        let capability = self
            .capability
            .resolve(resolver)
            .map_err(PropertyInterfaceRecordResolutionError::Capability)?;
        PropertyInterfaceRecordV1::try_new(
            declaration,
            owner,
            type_parameters,
            receiver,
            value_type,
            capability,
            self.representation,
            self.access,
        )
        .map_err(PropertyInterfaceRecordResolutionError::Record)
    }
}

impl WireEncode for DecodedPropertyInterfaceRecordV1 {
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
        self.capability.encode(encoder)?;
        encoder.field(7)?;
        self.representation.encode(encoder)?;
        encoder.field(8)?;
        self.access.encode(encoder)
    }
}

impl WireDecode for DecodedPropertyInterfaceRecordV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(8)?;
        Ok(Self {
            declaration: decoder.field(1, DecodedPropertyDeclarationId::decode)?,
            owner: decoder.field(2, DecodedPublicDeclarationOwnerV1::decode)?,
            type_parameters: decoder.field(3, DecodedCanonicalBinderListV1::decode)?,
            receiver: decoder.field(4, DecodedOptionalSignatureType::decode)?,
            value_type: decoder.field(5, DecodedSignatureTypeKey::decode)?,
            capability: decoder.field(6, DecodedPropertyCapabilityV1::decode)?,
            representation: decoder.field(7, PropertyRepresentationV1::decode)?,
            access: decoder.field(8, PropertyPublicAccessV1::decode)?,
        })
    }
}

pub trait PropertyInterfaceRecordResolver<E>:
    PropertyDeclarationIdResolver<E>
    + SignatureTypeReferenceResolver<E>
    + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
{
}

impl<R, E> PropertyInterfaceRecordResolver<E> for R where
    R: PropertyDeclarationIdResolver<E>
        + SignatureTypeReferenceResolver<E>
        + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
{
}

fn validate_declaration_shape(
    declaration: PropertyDeclarationId,
    owner: PublicDeclarationOwnerV1,
    type_parameters: &CanonicalBinderListV1,
    has_receiver: bool,
) -> Result<(), PropertyInterfaceRecordBuildError> {
    match declaration {
        PropertyOwner::Property(_) => {
            if owner == PublicDeclarationOwnerV1::Extension {
                return Err(PropertyInterfaceRecordBuildError::UnexpectedExtensionOwner(
                    declaration,
                ));
            }
            if !type_parameters.is_empty() {
                return Err(PropertyInterfaceRecordBuildError::UnexpectedTypeParameters(
                    declaration,
                ));
            }
            if has_receiver {
                return Err(PropertyInterfaceRecordBuildError::UnexpectedReceiver(owner));
            }
        }
        PropertyOwner::ExtensionProperty(_) => {
            if owner != PublicDeclarationOwnerV1::Extension {
                return Err(PropertyInterfaceRecordBuildError::ExtensionOwnerRequired {
                    declaration,
                    actual: owner,
                });
            }
            if !has_receiver {
                return Err(PropertyInterfaceRecordBuildError::MissingExtensionReceiver);
            }
        }
    }
    Ok(())
}

fn validate_access_shape(
    declaration: PropertyDeclarationId,
    owner: PublicDeclarationOwnerV1,
    access: PropertyPublicAccessV1,
) -> Result<(), PropertyInterfaceRecordBuildError> {
    if !matches!(owner, PublicDeclarationOwnerV1::Nominal(_))
        && access != PropertyPublicAccessV1::DirectOnly
    {
        return Err(PropertyInterfaceRecordBuildError::DirectPropertyContract {
            declaration,
            owner,
            access,
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_representation_shape(
    declaration: PropertyDeclarationId,
    owner: PublicDeclarationOwnerV1,
    type_parameters: &CanonicalBinderListV1,
    has_receiver: bool,
    capability: PropertyCapabilityV1,
    representation: PropertyRepresentationV1,
    access: PropertyPublicAccessV1,
) -> Result<(), PropertyInterfaceRecordBuildError> {
    match representation {
        PropertyRepresentationV1::Const => {
            if !capability.is_read_only() {
                return Err(PropertyInterfaceRecordBuildError::ConstMustBeReadOnly(
                    declaration,
                ));
            }
            if !type_parameters.is_empty() {
                return Err(PropertyInterfaceRecordBuildError::ConstCannotBeGeneric(
                    declaration,
                ));
            }
            if has_receiver {
                return Err(PropertyInterfaceRecordBuildError::ConstCannotHaveReceiver(
                    declaration,
                ));
            }
            if access != PropertyPublicAccessV1::DirectOnly {
                return Err(PropertyInterfaceRecordBuildError::ConstMustBeDirect(
                    declaration,
                ));
            }
        }
        PropertyRepresentationV1::AbstractSlot => {
            if !matches!(owner, PublicDeclarationOwnerV1::Nominal(_)) {
                return Err(
                    PropertyInterfaceRecordBuildError::AbstractNominalOwnerRequired {
                        declaration,
                        actual: owner,
                    },
                );
            }
            if access != PropertyPublicAccessV1::PublicSlot {
                return Err(
                    PropertyInterfaceRecordBuildError::AbstractSlotAccessRequired(declaration),
                );
            }
        }
        PropertyRepresentationV1::RuntimeAccessor => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests;
