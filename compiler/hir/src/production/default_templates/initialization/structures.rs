use super::*;
use crate::{
    DefaultConstructorRefV1, ExportConstructorDelegationV1, ExportConstructorInitializationKindV1,
    ExportConstructorInitializationV1, StructConstructorKind,
};

pub(super) fn project(
    entities: &DefaultEntityProjector<'_>,
    owner: scoop_identity::PersistentGenericTypeId,
    structure: crate::StructId,
) -> Result<ExportGenericNominalInitializationV1, Error> {
    let export = entities.export();
    let declaration = &export.structs[structure];
    let owner_type = export.struct_applications[declaration.self_application].canonical_type;
    let constructors = declaration
        .constructors
        .iter()
        .map(|&id| {
            let constructor = &export.struct_constructors[id];
            let projection = ConstructorProjection::new(
                entities,
                &constructor.parameters,
                &declaration.type_params,
                owner_type,
                constructor.origin,
            )?;
            let kind = match &constructor.kind {
                StructConstructorKind::Primary => {
                    ExportConstructorInitializationKindV1::StructPrimary
                }
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
        })
        .collect::<Result<_, Error>>()?;
    ExportGenericNominalInitializationV1::try_new(
        owner,
        Vec::new(),
        constructors,
        crate::ReleasePolicy::None,
    )
    .map_err(Error::Initialization)
}
