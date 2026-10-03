use scoop_identity::{
    DecodedOptionalSignatureType, DecodedSignatureTypeKey, OptionalSignatureType,
    PersistentIdResolver, PersistentPropertyAccessorId, PropertyOwner, SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    DecodedPropertyAccessorsV1, PropertyAccessorsV1, PropertyCapabilityV1, PropertyPublicAccessV1,
    PropertyRepresentationV1, PropertySetterPublicAccessV1,
};
use crate::{
    CanonicalBinderListV1, DeclaredVisibilityV1, DecodedCanonicalBinderListV1,
    DecodedPropertyDeclarationId, DecodedPublicDeclarationOwnerV1, PropertyDeclarationId,
    PropertyDeclarationIdResolver, PublicDeclarationOwnerV1, SignatureTypeReferenceResolver,
};

mod errors;
mod public;
mod semantics;
mod validation;
mod wire;

pub use public::{DecodedPropertyInterfaceRecordV1, PropertyInterfaceRecordV1};
use validation::*;
pub use wire::DecodedPropertyDeclarationRecordV1;

pub use errors::{PropertyInterfaceRecordBuildError, PropertyInterfaceRecordResolutionError};
pub use semantics::{
    PropertyDeclarationIdentityShapeV1, PropertyDeclarationSourceShapeV1,
    PropertyInterfaceSemanticAuthority, PropertyInterfaceSemanticValidationError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropertyDeclarationRecordV1 {
    declaration: PropertyDeclarationId,
    owner: PublicDeclarationOwnerV1,
    type_parameters: CanonicalBinderListV1,
    receiver: Option<SignatureTypeKey>,
    value_type: SignatureTypeKey,
    accessors: PropertyAccessorsV1,
    representation: PropertyRepresentationV1,
    visibility: DeclaredVisibilityV1,
}

impl PropertyDeclarationRecordV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        declaration: PropertyDeclarationId,
        owner: PublicDeclarationOwnerV1,
        type_parameters: CanonicalBinderListV1,
        receiver: Option<SignatureTypeKey>,
        value_type: SignatureTypeKey,
        accessors: PropertyAccessorsV1,
        representation: PropertyRepresentationV1,
        visibility: DeclaredVisibilityV1,
    ) -> Result<Self, PropertyInterfaceRecordBuildError> {
        validate_declaration_shape(declaration, owner, &type_parameters, receiver.is_some())?;
        validate_source_representation(
            declaration,
            owner,
            &type_parameters,
            receiver.is_some(),
            accessors,
            representation,
        )?;
        Ok(Self {
            declaration,
            owner,
            type_parameters,
            receiver,
            value_type,
            accessors,
            representation,
            visibility,
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
    pub const fn accessors(&self) -> PropertyAccessorsV1 {
        self.accessors
    }
    pub const fn representation(&self) -> PropertyRepresentationV1 {
        self.representation
    }
    pub const fn declared_visibility(&self) -> DeclaredVisibilityV1 {
        self.visibility
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

#[cfg(test)]
mod tests;
