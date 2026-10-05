use scoop_identity::{
    CallableTemplateOrigin, DefinitionOwnerAtom, DuplicateSignatureKey, NominalDeclarationOwner,
    SourceDeclarationKind,
};

use super::{
    CallableInterfaceBuildError, CallableProjection, CallableProjectionError,
    CallableProjectionSubject, access, effects, parameters,
};
use crate::{
    CallableDeclarationRecordV1, CallableInterfaceRecordV1, CallableModalityV1,
    ClassConstructorIdentityKind, HirClassConstructorIdentity, HirSourceNominalIdentity,
    PublicDeclarationOwnerV1, TypeParamDecl,
};

pub(super) fn project_all(
    projection: &CallableProjection<'_>,
    records: &mut Vec<CallableInterfaceRecordV1>,
) -> Result<(), CallableInterfaceBuildError> {
    for &constructor in &projection.export.public_surface.struct_constructors {
        let subject = CallableProjectionSubject::StructConstructor(super::raw_index(constructor));
        records.push(
            project_struct(projection, constructor)
                .and_then(|data| {
                    let access = access::project_direct(
                        &projection.export.struct_constructors[constructor].access,
                    )
                    .map_err(CallableProjectionError::Access)?;
                    CallableInterfaceRecordV1::from_declaration(data, access)
                        .map_err(CallableProjectionError::Record)
                })
                .map_err(|error| CallableInterfaceBuildError::projection(subject, error))?,
        );
    }
    for &constructor in &projection.export.public_surface.class_constructors {
        let subject = CallableProjectionSubject::ClassConstructor(super::raw_index(constructor));
        let Some(record) = project_class(projection, constructor)
            .map_err(|error| CallableInterfaceBuildError::projection(subject, error))?
        else {
            continue;
        };
        let access =
            access::project_direct(&projection.export.class_constructors[constructor].access)
                .map_err(|error| {
                    CallableInterfaceBuildError::projection(
                        subject,
                        CallableProjectionError::Access(error),
                    )
                })?;
        records.push(
            CallableInterfaceRecordV1::from_declaration(record, access).map_err(|error| {
                CallableInterfaceBuildError::projection(
                    subject,
                    CallableProjectionError::Record(error),
                )
            })?,
        );
    }
    Ok(())
}

pub(super) fn project_struct(
    projection: &CallableProjection<'_>,
    constructor_id: crate::StructConstructorId,
) -> Result<CallableDeclarationRecordV1, CallableProjectionError> {
    let constructor = super::arena_get(&projection.export.struct_constructors, constructor_id)
        .ok_or(CallableProjectionError::UnknownDeclaration)?;
    let owner = super::arena_get(&projection.export.structs, constructor.owner)
        .ok_or(CallableProjectionError::InvalidOwner)?;
    if !owner.constructors.contains(&constructor_id) {
        return Err(CallableProjectionError::ConstructorOwnerMismatch);
    }
    let identity = projection
        .export
        .constructor_identities
        .get_struct(constructor_id)
        .ok_or(CallableProjectionError::MissingIdentity)?;
    project_source(
        projection,
        CallableTemplateOrigin::Constructor(identity.id()),
        identity.key(),
        crate::ExportParameterOwner::StructConstructor(constructor_id),
        source_owner(
            projection,
            projection
                .export
                .nominal_identities
                .get_struct(constructor.owner),
        )?,
        &owner.type_params,
        super::arena_get(
            &projection.export.struct_applications,
            owner.self_application,
        )
        .ok_or(CallableProjectionError::InvalidOwner)?
        .canonical_type,
        &constructor.access,
        constructor.safety,
        constructor.source_gc_effect(),
        &constructor.release_callability,
    )
}

pub(super) fn project_class(
    projection: &CallableProjection<'_>,
    constructor_id: crate::ClassConstructorId,
) -> Result<Option<CallableDeclarationRecordV1>, CallableProjectionError> {
    let constructor = super::arena_get(&projection.export.class_constructors, constructor_id)
        .ok_or(CallableProjectionError::UnknownDeclaration)?;
    let owner = super::arena_get(&projection.export.classes, constructor.owner)
        .ok_or(CallableProjectionError::InvalidOwner)?;
    if !owner.constructors.contains(&constructor_id) {
        return Err(CallableProjectionError::ConstructorOwnerMismatch);
    }
    let identity = projection
        .export
        .constructor_identities
        .get_class(constructor_id)
        .ok_or(CallableProjectionError::MissingIdentity)?;
    let HirClassConstructorIdentity::Source(identity) = identity else {
        if !matches!(
            constructor.identity_kind,
            ClassConstructorIdentityKind::ZeroArgumentAdapter { .. }
        ) {
            return Err(CallableProjectionError::GeneratedConstructor);
        }
        return Ok(None);
    };
    if constructor.identity_kind != ClassConstructorIdentityKind::Source {
        return Err(CallableProjectionError::InvalidIdentityKind);
    }
    project_source(
        projection,
        CallableTemplateOrigin::Constructor(identity.id()),
        identity.key(),
        crate::ExportParameterOwner::ClassConstructor(constructor_id),
        source_owner(
            projection,
            projection
                .export
                .nominal_identities
                .get_class(constructor.owner)
                .filter(|identity| identity.source().is_some())
                .or_else(|| {
                    projection
                        .export
                        .objects
                        .iter()
                        .find(|(_, object)| object.backing_class == constructor.owner)
                        .map(|(id, _)| &projection.export.nominal_identities[id])
                }),
        )?,
        &owner.type_params,
        super::arena_get(
            &projection.export.class_applications,
            owner.self_application,
        )
        .ok_or(CallableProjectionError::InvalidOwner)?
        .canonical_type,
        &constructor.access,
        constructor.safety,
        crate::GcEffect::Managed,
        &crate::ReleaseCallability::Unavailable,
    )
    .map(Some)
}

#[allow(clippy::too_many_arguments)]
fn project_source(
    projection: &CallableProjection<'_>,
    declaration: CallableTemplateOrigin,
    key: &scoop_identity::SourceDeclarationKey,
    parameter_owner: crate::ExportParameterOwner,
    owner: NominalDeclarationOwner,
    owner_parameters: &[TypeParamDecl],
    result_type: crate::TypeId,
    declaration_access: &crate::DeclarationAccess,
    safety: crate::Safety,
    gc_effect: crate::GcEffect,
    release_callability: &crate::ReleaseCallability,
) -> Result<CallableDeclarationRecordV1, CallableProjectionError> {
    validate_source_key(projection, key, owner)?;
    let DuplicateSignatureKey::Constructor {
        parameters: identity_parameters,
    } = key.duplicate_signature()
    else {
        return Err(CallableProjectionError::InvalidDeclarationKind {
            expected: SourceDeclarationKind::Constructor,
            actual: key.declaration_kind(),
        });
    };
    let binders = projection
        .signatures
        .binder_frame(owner_parameters, 0)
        .map_err(CallableProjectionError::Signature)?;
    let type_parameters = projection
        .signatures
        .project_binder_list(&[], &binders)
        .map_err(CallableProjectionError::Signature)?;
    let parameters =
        parameters::project(projection, parameter_owner, &binders, identity_parameters)
            .map_err(CallableProjectionError::Parameters)?;
    let result = projection
        .signatures
        .map_type(result_type, &binders)
        .map_err(CallableProjectionError::Signature)?;
    let effects = effects::source_constructor(safety, gc_effect, release_callability, &binders)
        .map_err(CallableProjectionError::Effects)?;
    CallableDeclarationRecordV1::try_new(
        declaration,
        PublicDeclarationOwnerV1::Nominal(owner),
        type_parameters,
        None,
        parameters,
        result,
        effects,
        CallableModalityV1::Final,
        declaration_access.declared.into(),
        crate::CanonicalPersistentIdsV1::empty(),
        Vec::new(),
    )
    .map_err(CallableProjectionError::Record)
}

fn validate_source_key(
    projection: &CallableProjection<'_>,
    key: &scoop_identity::SourceDeclarationKey,
    owner: NominalDeclarationOwner,
) -> Result<(), CallableProjectionError> {
    if key.origin() != projection.export.cone {
        return Err(CallableProjectionError::ForeignDeclaration {
            expected: projection.export.cone,
            actual: key.origin(),
        });
    }
    if key.declaration_kind() != SourceDeclarationKind::Constructor {
        return Err(CallableProjectionError::InvalidDeclarationKind {
            expected: SourceDeclarationKind::Constructor,
            actual: key.declaration_kind(),
        });
    }
    if key.owners().owners().last() != Some(&owner_atom(owner)) {
        return Err(CallableProjectionError::PersistentOwnerMismatch);
    }
    Ok(())
}

fn source_owner(
    projection: &CallableProjection<'_>,
    identity: Option<&crate::HirNominalIdentity>,
) -> Result<NominalDeclarationOwner, CallableProjectionError> {
    let source = identity
        .and_then(crate::HirNominalIdentity::source)
        .ok_or(CallableProjectionError::MissingNominalOwner)?;
    if source.declaration().origin() != projection.export.cone {
        return Err(CallableProjectionError::ForeignNominalOwner {
            expected: projection.export.cone,
            actual: source.declaration().origin(),
        });
    }
    Ok(match source {
        HirSourceNominalIdentity::Concrete(record) => {
            NominalDeclarationOwner::Concrete(record.id())
        }
        HirSourceNominalIdentity::Generic(record) => {
            NominalDeclarationOwner::GenericTemplate(record.id())
        }
    })
}

fn owner_atom(owner: NominalDeclarationOwner) -> DefinitionOwnerAtom {
    match owner {
        NominalDeclarationOwner::Concrete(id) => DefinitionOwnerAtom::Type(id),
        NominalDeclarationOwner::GenericTemplate(id) => DefinitionOwnerAtom::GenericType(id),
    }
}
