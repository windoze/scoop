use super::*;

pub(super) fn validate_type_parameter_shape(
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

pub(super) fn validate_receiver_shape(
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

pub(super) fn validate_owner_shape(
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

pub(super) fn validate_dispatch_shape(
    declaration: CallableTemplateOrigin,
    owner: PublicDeclarationOwnerV1,
    effects: &CallableSourceEffectsV1,
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
    ) || matches!(
        effects.implementation(),
        CallableImplementationV1::SourceExternScoop | CallableImplementationV1::SourceExternC(..)
    );
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
        CallableImplementationV1::SourceExternScoop | CallableImplementationV1::SourceExternC(..)
    ) && (!matches!(declaration, CallableTemplateOrigin::Function(_))
        || owner != PublicDeclarationOwnerV1::TopLevel)
    {
        return Err(CallableInterfaceRecordBuildError::InvalidExternTarget { declaration, owner });
    }
    Ok(())
}

pub(super) fn validate_source_dispatch(
    declaration: CallableTemplateOrigin,
    owner: PublicDeclarationOwnerV1,
    effects: &CallableSourceEffectsV1,
    modality: CallableModalityV1,
    visibility: DeclaredVisibilityV1,
    slots: &CanonicalPersistentIdsV1<PersistentDispatchSlotId>,
) -> Result<(), CallableInterfaceRecordBuildError> {
    let access = if slots.is_empty() {
        PublicLookupAccessV1::DirectOnly
    } else {
        PublicLookupAccessV1::PublicSlot
    };
    validate_dispatch_shape(declaration, owner, effects, modality, access)?;
    if slots.values().len() > 1
        || (visibility == DeclaredVisibilityV1::Private && !slots.is_empty())
        || (matches!(declaration, CallableTemplateOrigin::GenericFunction(_)) && !slots.is_empty())
    {
        return Err(CallableInterfaceRecordBuildError::InvalidSlotRelations { declaration });
    }
    Ok(())
}
