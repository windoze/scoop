use super::*;
use scoop_identity::{CanonicalIdentifier, DeclarationName};

pub(super) fn project(
    export: &ExportHir,
    local: LocalNominalId,
    source: &HirSourceNominalIdentity,
) -> Result<NominalInterfaceRecordV1, Error> {
    let owner = source_nominal_id(source);
    let key = source.declaration();
    let (name, lexical_owner, parameters) = header(export, local);

    let name = CanonicalIdentifier::new(name).map_err(invalid)?;
    let lexical_owner = lexical_owner
        .map(|owner| source_owner(export, owner))
        .transpose()?;
    if key.declaration_kind() != local.kind().source_kind()
        || key.name() != &DeclarationName::Named(name)
        || key.owners().owners().last() != lexical_owner.map(owner_atom).as_ref()
        || usize::try_from(key.duplicate_signature().type_parameter_count()).ok()
            != Some(parameters.len())
    {
        return Err(invalid(
            "nominal source identity disagrees with its sealed HIR declaration",
        ));
    }

    let signatures = HirInterfaceSignatureProjector::new(export);
    let binders = signatures.binder_frame(parameters, 0).map_err(invalid)?;
    let type_parameters = signatures
        .project_binder_list(parameters, &binders)
        .map_err(invalid)?;
    let (supertypes, shape) = shapes::project(export, local, owner, &binders)?;
    let visibility = match local {
        LocalNominalId::Class(id) => export.classes[id].access.declared,
        LocalNominalId::Interface(id) => export.interfaces[id].access.declared,
        LocalNominalId::Struct(id) => export.structs[id].access.declared,
        LocalNominalId::Enum(id) => export.enums[id].access.declared,
        LocalNominalId::Object(id) => export.objects[id].access.declared,
    };
    let primary = if let LocalNominalId::Struct(id) = local {
        let constructors = &export.structs[id].constructors;
        if constructors.is_empty() {
            None
        } else {
            let primary = constructors
                .iter()
                .find(|&&constructor| {
                    matches!(
                        export.struct_constructors[constructor].kind,
                        StructConstructorKind::Primary
                    )
                })
                .ok_or_else(|| invalid("struct constructor set has no primary constructor"))?;
            Some(export.constructor_identities[*primary].id())
        }
    } else {
        None
    };
    let details = NominalDeclarationDetailsV1::new(
        modality(export, local),
        visibility.into(),
        constructors::project(export, local, owner)?,
        members::project(export, local, owner)?,
        children(export, owner)?,
        dispatch_order::project(export, local)?,
        crate::production::nominal_dispatch::project(export, local.owner())?,
        primary,
        instantiation_conditions(export, local, parameters)?,
        release_policy(export, local, parameters)?,
    );
    NominalInterfaceRecordV1::try_new(
        owner,
        shape.kind(),
        type_parameters,
        supertypes,
        CanonicalPersistentIdsV1::empty(),
        CanonicalPublicMemberRefsV1::default(),
        CanonicalPersistentIdsV1::empty(),
        shape,
        details,
    )
    .map_err(invalid)
}

pub(super) fn header(
    export: &ExportHir,
    local: LocalNominalId,
) -> (&str, Option<NominalOwner>, &[TypeParamDecl]) {
    match local {
        LocalNominalId::Class(id) => {
            let d = &export.classes[id];
            (&d.name, d.owner, &d.type_params)
        }
        LocalNominalId::Interface(id) => {
            let d = &export.interfaces[id];
            (&d.name, d.owner, &d.type_params)
        }
        LocalNominalId::Struct(id) => {
            let d = &export.structs[id];
            (&d.name, d.owner, &d.type_params)
        }
        LocalNominalId::Enum(id) => {
            let d = &export.enums[id];
            (&d.name, d.owner, &d.type_params)
        }
        LocalNominalId::Object(id) => {
            let d = &export.objects[id];
            (&d.name, d.owner, &[])
        }
    }
}
fn modality(export: &ExportHir, local: LocalNominalId) -> NominalInheritanceModalityV1 {
    match local {
        LocalNominalId::Class(id) => match export.classes[id].modifier {
            ClassModifier::Final => NominalInheritanceModalityV1::Final,
            ClassModifier::Open => NominalInheritanceModalityV1::Open,
            ClassModifier::Abstract => NominalInheritanceModalityV1::Abstract,
        },
        LocalNominalId::Interface(_) => NominalInheritanceModalityV1::Interface,
        LocalNominalId::Struct(_) | LocalNominalId::Enum(_) | LocalNominalId::Object(_) => {
            NominalInheritanceModalityV1::Final
        }
    }
}

fn instantiation_conditions(
    export: &ExportHir,
    local: LocalNominalId,
    parameters: &[TypeParamDecl],
) -> Result<crate::NominalInstantiationConditionsV1, Error> {
    let (no_gc, requirements): (_, &[crate::RequiresGcFreePointee]) = match local {
        LocalNominalId::Struct(id) => (
            export.structs[id].attributes.no_gc,
            &export.structs[id].gc_free_pointee_requirements,
        ),
        LocalNominalId::Enum(id) => (
            export.enums[id].no_gc,
            &export.enums[id].gc_free_pointee_requirements,
        ),
        LocalNominalId::Class(id) => (false, &export.classes[id].gc_free_pointee_requirements),
        LocalNominalId::Interface(id) => {
            (false, &export.interfaces[id].gc_free_pointee_requirements)
        }
        LocalNominalId::Object(_) => (false, &[]),
    };
    let mut indices = requirements
        .iter()
        .map(|requirement| {
            parameters
                .iter()
                .position(|parameter| parameter.id == requirement.type_param)
                .and_then(|index| u32::try_from(index).ok())
                .ok_or_else(|| {
                    invalid("nominal pointee condition must name its original parameter")
                })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    indices.sort_unstable();
    let arguments = indices
        .into_iter()
        .map(|index| scoop_identity::SignatureTypeKey::Binder { depth: 0, index })
        .collect();
    Ok(crate::NominalInstantiationConditionsV1::new(
        no_gc,
        crate::CanonicalBinderUseListV1::try_new(arguments).map_err(invalid)?,
    ))
}

fn release_policy(
    export: &ExportHir,
    local: LocalNominalId,
    parameters: &[TypeParamDecl],
) -> Result<crate::NominalReleasePolicyV1, Error> {
    let LocalNominalId::Class(id) = local else {
        return Ok(crate::NominalReleasePolicyV1::None);
    };
    let requirements = match &export.classes[id].release_policy {
        crate::ReleasePolicy::None => return Ok(crate::NominalReleasePolicyV1::None),
        crate::ReleasePolicy::SynchronousGcFree { hook } => match hook {
            crate::ExportReleaseHookRef::Template(id) => &export.release_hooks[*id].requirements,
            crate::ExportReleaseHookRef::Imported { requirements } => requirements,
        },
    };
    let mut requirements = requirements
        .iter()
        .map(|id| {
            let index = parameters
                .iter()
                .position(|parameter| parameter.id == *id)
                .and_then(|index| u32::try_from(index).ok())
                .ok_or_else(|| {
                    invalid("release condition must name its original nominal parameter")
                })?;
            Ok(crate::ReleaseValueBinderV1 { depth: 0, index })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    requirements.sort_unstable();
    Ok(crate::NominalReleasePolicyV1::SynchronousGcFree { requirements })
}
