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
