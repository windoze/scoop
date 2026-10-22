use super::*;
use crate::{
    DefaultConstructorRefV1, ExportConstructorDelegationV1, ExportConstructorInitializationKindV1,
    ExportConstructorInitializationV1, StructConstructorKind,
};

pub(super) fn project(
    export: &ExportHir,
    entities: &DefaultEntityProjector<'_>,
    owner: scoop_identity::PersistentGenericTypeId,
    structure: crate::StructId,
) -> Result<ExportGenericNominalInitializationV1, Error> {
    let constructors = export.structs[structure]
        .constructors
        .iter()
        .map(|&id| project_constructor(entities, id))
        .collect::<Result<_, _>>()?;
    ExportGenericNominalInitializationV1::try_new(
        owner,
        Vec::new(),
        constructors,
        crate::ReleasePolicy::None,
    )
    .map_err(Error::Initialization)
}

pub(in crate::production::default_templates) fn project_constructor(
    entities: &DefaultEntityProjector<'_>,
    id: crate::StructConstructorId,
) -> Result<ExportConstructorInitializationV1, Error> {
    let inputs = entities.inputs();
    let constructor = &inputs.struct_constructors[id];
    let declaration = &inputs.type_inputs.structs[constructor.owner];
    let owner_type = inputs.struct_applications[declaration.self_application].canonical_type;
    let projection = ConstructorProjection::new(
        entities,
        &constructor.parameters,
        None,
        &declaration.type_params,
        owner_type,
        constructor.origin,
    )?;
    let kind = match &constructor.kind {
        StructConstructorKind::Primary => ExportConstructorInitializationKindV1::StructPrimary,
        StructConstructorKind::Secondary {
            delegation, body, ..
        } => ExportConstructorInitializationKindV1::StructSecondary {
            delegation: ExportConstructorDelegationV1 {
                target: entities
                    .struct_constructor(delegation.target, &projection.binders)
                    .map_err(Error::Entity)?,
                arguments: projection.arguments(&delegation.arguments)?,
            },
            body: projection.body(body)?,
        },
    };
    let effects = crate::production::callable_interfaces::source_constructor_effects(
        constructor.safety,
        constructor.source_gc_effect(),
        &constructor.release_callability,
        &projection.binders,
    )
    .map_err(Error::Effects)?;
    ExportConstructorInitializationV1::try_new(
        DefaultConstructorRefV1::Struct {
            declaration: entities.struct_constructor_id(id).map_err(Error::Entity)?,
            owner_type: projection.owner.clone(),
        },
        projection.inputs.clone(),
        effects,
        projection.predicates(
            &constructor.no_gc_type_params,
            &declaration.gc_free_pointee_requirements,
        )?,
        projection.definition_source()?,
        kind,
    )
    .map_err(Error::Initialization)
}
