use super::*;
use scoop_identity::{CanonicalIdentifier, DeclarationName};

pub(super) fn project(
    export: &ExportHir,
    local: LocalNominalId,
    source: &HirSourceNominalIdentity,
    meter: &mut BudgetMeter,
) -> Result<NominalSourceContractV1, Error> {
    let owner = source_nominal_id(source);
    let key = source.declaration();
    let (name, lexical_owner, parameters) = header(export, local);
    resources::name(name, meter)?;
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
    resources::binders(export, parameters, parameters.len(), meter)?;
    let signatures = HirInterfaceSignatureProjector::new(export);
    let binders = signatures.binder_frame(parameters, 0).map_err(invalid)?;
    let type_parameters = signatures
        .project_binder_list(parameters, &binders)
        .map_err(invalid)?;
    let (supertypes, shape) = shapes::project(export, local, owner, &binders, meter)?;
    NominalSourceContractV1::try_new(
        owner,
        modality(export, local),
        type_parameters,
        supertypes,
        constructors::project(export, local, owner, meter)?,
        members::project(export, local, owner, meter)?,
        children(export, owner, meter)?,
        shape,
    )
    .map_err(Error::SourceInventory)
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
