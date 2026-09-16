use scoop_identity::{
    CallableTemplateOrigin, DecodedCallableTemplateOrigin, DecodedOptionalSignatureType,
    DecodedSignatureTypeKey, OptionalSignatureType, SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    CallableImplementationV1, CallableModalityV1, CallableSourceEffectsV1,
    CanonicalSourceParameterShapesV1, DecodedCallableSourceEffectsV1,
    DecodedCanonicalSourceParameterShapesV1, PublicLookupAccessV1,
};
use crate::{
    BinderListValidationError, CallableDeclarationIdResolver, CanonicalBinderListV1,
    DecodedCanonicalBinderListV1, DecodedPublicDeclarationOwnerV1, PublicDeclarationOwnerV1,
    SignatureTypeReferenceResolver, SourceParameterListValidationError,
};

mod errors;
mod semantics;

pub use errors::{CallableInterfaceRecordBuildError, CallableInterfaceRecordResolutionError};
pub use semantics::{
    CallableDeclarationIdentityShapeV1, CallableInterfaceSemanticAuthority,
    CallableInterfaceSemanticValidationError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableInterfaceRecordV1 {
    declaration: CallableTemplateOrigin,
    owner: PublicDeclarationOwnerV1,
    type_parameters: CanonicalBinderListV1,
    receiver: Option<SignatureTypeKey>,
    parameters: CanonicalSourceParameterShapesV1,
    result: SignatureTypeKey,
    effects: CallableSourceEffectsV1,
    modality: CallableModalityV1,
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
    ) -> Result<Self, CallableInterfaceRecordBuildError> {
        validate_type_parameter_shape(declaration, &type_parameters)?;
        validate_receiver_shape(owner, receiver.is_some())?;
        validate_owner_shape(declaration, owner)?;
        validate_dispatch_shape(declaration, owner, effects, modality, access)?;
        Ok(Self {
            declaration,
            owner,
            type_parameters,
            receiver,
            parameters,
            result,
            effects,
            modality,
            access,
        })
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

    pub const fn effects(&self) -> CallableSourceEffectsV1 {
        self.effects
    }

    pub const fn modality(&self) -> CallableModalityV1 {
        self.modality
    }

    pub const fn access(&self) -> PublicLookupAccessV1 {
        self.access
    }
}

impl WireEncode for CallableInterfaceRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
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
        self.access.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCallableInterfaceRecordV1 {
    declaration: DecodedCallableTemplateOrigin,
    owner: DecodedPublicDeclarationOwnerV1,
    type_parameters: DecodedCanonicalBinderListV1,
    receiver: DecodedOptionalSignatureType,
    parameters: DecodedCanonicalSourceParameterShapesV1,
    result: DecodedSignatureTypeKey,
    effects: DecodedCallableSourceEffectsV1,
    modality: CallableModalityV1,
    access: PublicLookupAccessV1,
}

impl DecodedCallableInterfaceRecordV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CallableInterfaceRecordV1, CallableInterfaceRecordResolutionError<E>>
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
        CallableInterfaceRecordV1::try_new(
            declaration,
            owner,
            type_parameters,
            receiver,
            parameters,
            result,
            effects,
            self.modality,
            self.access,
        )
        .map_err(CallableInterfaceRecordResolutionError::Record)
    }
}

impl WireEncode for DecodedCallableInterfaceRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
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
        self.access.encode(encoder)
    }
}

impl WireDecode for DecodedCallableInterfaceRecordV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(9)?;
        Ok(Self {
            declaration: decoder.field(1, DecodedCallableTemplateOrigin::decode)?,
            owner: decoder.field(2, DecodedPublicDeclarationOwnerV1::decode)?,
            type_parameters: decoder.field(3, DecodedCanonicalBinderListV1::decode)?,
            receiver: decoder.field(4, DecodedOptionalSignatureType::decode)?,
            parameters: decoder.field(5, DecodedCanonicalSourceParameterShapesV1::decode)?,
            result: decoder.field(6, DecodedSignatureTypeKey::decode)?,
            effects: decoder.field(7, DecodedCallableSourceEffectsV1::decode)?,
            modality: decoder.field(8, CallableModalityV1::decode)?,
            access: decoder.field(9, PublicLookupAccessV1::decode)?,
        })
    }
}

pub trait CallableInterfaceRecordResolver<E>:
    CallableDeclarationIdResolver<E> + SignatureTypeReferenceResolver<E>
{
}

impl<R, E> CallableInterfaceRecordResolver<E> for R where
    R: CallableDeclarationIdResolver<E> + SignatureTypeReferenceResolver<E>
{
}

fn validate_type_parameter_shape(
    declaration: CallableTemplateOrigin,
    type_parameters: &CanonicalBinderListV1,
) -> Result<(), CallableInterfaceRecordBuildError> {
    match declaration {
        CallableTemplateOrigin::GenericFunction(_) if type_parameters.is_empty() => Err(
            CallableInterfaceRecordBuildError::MissingTypeParameters(declaration),
        ),
        CallableTemplateOrigin::Function(_)
        | CallableTemplateOrigin::Constructor(_)
        | CallableTemplateOrigin::Accessor(_)
        | CallableTemplateOrigin::VariantConstructor(_)
            if !type_parameters.is_empty() =>
        {
            Err(CallableInterfaceRecordBuildError::UnexpectedTypeParameters(
                declaration,
            ))
        }
        CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::GenericFunction(_) => Ok(()),
        CallableTemplateOrigin::Constructor(_)
        | CallableTemplateOrigin::Accessor(_)
        | CallableTemplateOrigin::VariantConstructor(_) => Ok(()),
    }
}

fn validate_receiver_shape(
    owner: PublicDeclarationOwnerV1,
    has_receiver: bool,
) -> Result<(), CallableInterfaceRecordBuildError> {
    match (owner, has_receiver) {
        (PublicDeclarationOwnerV1::Extension, false) => {
            Err(CallableInterfaceRecordBuildError::MissingExtensionReceiver)
        }
        (PublicDeclarationOwnerV1::TopLevel | PublicDeclarationOwnerV1::Nominal(_), true) => {
            Err(CallableInterfaceRecordBuildError::UnexpectedReceiver(owner))
        }
        (PublicDeclarationOwnerV1::Extension, true)
        | (PublicDeclarationOwnerV1::TopLevel | PublicDeclarationOwnerV1::Nominal(_), false) => {
            Ok(())
        }
    }
}

fn validate_owner_shape(
    declaration: CallableTemplateOrigin,
    owner: PublicDeclarationOwnerV1,
) -> Result<(), CallableInterfaceRecordBuildError> {
    if matches!(
        declaration,
        CallableTemplateOrigin::Constructor(_) | CallableTemplateOrigin::VariantConstructor(_)
    ) && !matches!(owner, PublicDeclarationOwnerV1::Nominal(_))
    {
        return Err(CallableInterfaceRecordBuildError::NominalOwnerRequired {
            declaration,
            actual: owner,
        });
    }
    Ok(())
}

fn validate_dispatch_shape(
    declaration: CallableTemplateOrigin,
    owner: PublicDeclarationOwnerV1,
    effects: CallableSourceEffectsV1,
    modality: CallableModalityV1,
    access: PublicLookupAccessV1,
) -> Result<(), CallableInterfaceRecordBuildError> {
    if modality == CallableModalityV1::InterfaceDefault
        && !matches!(owner, PublicDeclarationOwnerV1::Nominal(_))
    {
        return Err(CallableInterfaceRecordBuildError::NominalOwnerRequired {
            declaration,
            actual: owner,
        });
    }
    if modality != CallableModalityV1::Final && access != PublicLookupAccessV1::PublicSlot {
        return Err(CallableInterfaceRecordBuildError::SlotAccessRequired(
            modality,
        ));
    }
    let direct_only = matches!(
        owner,
        PublicDeclarationOwnerV1::TopLevel | PublicDeclarationOwnerV1::Extension
    ) || matches!(
        declaration,
        CallableTemplateOrigin::Constructor(_) | CallableTemplateOrigin::VariantConstructor(_)
    ) || effects.implementation() != CallableImplementationV1::Scoop;
    if direct_only
        && (modality != CallableModalityV1::Final || access != PublicLookupAccessV1::DirectOnly)
    {
        return Err(CallableInterfaceRecordBuildError::DirectCallableContract {
            declaration,
            owner,
            modality,
            access,
        });
    }
    if matches!(
        effects.implementation(),
        CallableImplementationV1::SourceExternScoop | CallableImplementationV1::SourceExternC
    ) && (!matches!(declaration, CallableTemplateOrigin::Function(_))
        || owner != PublicDeclarationOwnerV1::TopLevel)
    {
        return Err(CallableInterfaceRecordBuildError::InvalidExternTarget { declaration, owner });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
