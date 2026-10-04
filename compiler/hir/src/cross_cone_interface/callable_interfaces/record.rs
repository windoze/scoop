use scoop_identity::{
    CallableTemplateOrigin, DecodedCallableTemplateOrigin, DecodedOptionalSignatureType,
    DecodedSignatureTypeKey, OptionalSignatureType, PersistentDispatchSlotId, PersistentIdResolver,
    SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    CallableImplementationV1, CallableModalityV1, CallableSourceEffectsV1,
    CanonicalSourceParameterShapesV1, DecodedCallableSourceEffectsV1,
    DecodedCanonicalSourceParameterShapesV1, PublicLookupAccessV1,
};
use crate::{
    BinderListValidationError, CallableDeclarationIdResolver, CanonicalBinderListV1,
    CanonicalPersistentIdsV1, DeclaredVisibilityV1, DecodedCanonicalBinderListV1,
    DecodedCanonicalPersistentIdsV1, DecodedPublicDeclarationOwnerV1, PublicDeclarationOwnerV1,
    SignatureTypeReferenceResolver, SourceParameterListValidationError,
};

mod errors;
mod public;
mod semantics;
mod validation;
mod wire;

pub use errors::{CallableInterfaceRecordBuildError, CallableInterfaceRecordResolutionError};
pub use public::{CallableInterfaceRecordV1, DecodedCallableInterfaceRecordV1};
pub use semantics::{
    CallableDeclarationIdentityShapeV1, CallableInterfaceSemanticAuthority,
    CallableInterfaceSemanticValidationError,
};
use validation::*;
pub use wire::DecodedCallableDeclarationRecordV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableDeclarationRecordV1 {
    declaration: CallableTemplateOrigin,
    owner: PublicDeclarationOwnerV1,
    type_parameters: CanonicalBinderListV1,
    receiver: Option<SignatureTypeKey>,
    parameters: CanonicalSourceParameterShapesV1,
    result: SignatureTypeKey,
    effects: CallableSourceEffectsV1,
    modality: CallableModalityV1,
    visibility: DeclaredVisibilityV1,
    slots: CanonicalPersistentIdsV1<PersistentDispatchSlotId>,
    context_parameters: Vec<crate::SourceParameterShapeV1>,
}

impl CallableDeclarationRecordV1 {
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
        visibility: DeclaredVisibilityV1,
        slots: CanonicalPersistentIdsV1<PersistentDispatchSlotId>,
        context_parameters: Vec<crate::SourceParameterShapeV1>,
    ) -> Result<Self, CallableInterfaceRecordBuildError> {
        validate_type_parameter_shape(declaration, &type_parameters)?;
        validate_receiver_shape(owner, receiver.is_some())?;
        validate_owner_shape(declaration, owner)?;
        validate_source_dispatch(declaration, owner, &effects, modality, visibility, &slots)?;
        Ok(Self {
            declaration,
            owner,
            type_parameters,
            receiver,
            parameters,
            result,
            effects,
            modality,
            visibility,
            slots,
            context_parameters,
        })
    }

    pub fn context_parameters(&self) -> &[crate::SourceParameterShapeV1] {
        &self.context_parameters
    }

    pub const fn declaration(&self) -> CallableTemplateOrigin {
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

    pub const fn parameters(&self) -> &CanonicalSourceParameterShapesV1 {
        &self.parameters
    }

    pub const fn result(&self) -> &SignatureTypeKey {
        &self.result
    }

    pub fn effects(&self) -> CallableSourceEffectsV1 {
        self.effects.clone()
    }

    pub const fn modality(&self) -> CallableModalityV1 {
        self.modality
    }

    pub const fn declared_visibility(&self) -> DeclaredVisibilityV1 {
        self.visibility
    }

    pub const fn slot_relations(&self) -> &CanonicalPersistentIdsV1<PersistentDispatchSlotId> {
        &self.slots
    }
}

pub trait CallableInterfaceRecordResolver<E>:
    CallableDeclarationIdResolver<E>
    + SignatureTypeReferenceResolver<E>
    + PersistentIdResolver<PersistentDispatchSlotId, Error = E>
{
}

impl<R, E> CallableInterfaceRecordResolver<E> for R where
    R: CallableDeclarationIdResolver<E>
        + SignatureTypeReferenceResolver<E>
        + PersistentIdResolver<PersistentDispatchSlotId, Error = E>
{
}

#[cfg(test)]
mod tests;
