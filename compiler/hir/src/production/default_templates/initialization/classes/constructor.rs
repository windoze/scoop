use super::*;

pub(in crate::production::default_templates) fn project(
    entities: &DefaultEntityProjector<'_>,
    id: crate::ClassConstructorId,
) -> Result<ExportConstructorInitializationV1, Error> {
    let inputs = entities.inputs();
    let constructor = &inputs.class_constructors[id];
    let declaration = &inputs.type_inputs.classes[constructor.owner];
    let owner_type = inputs.class_applications[declaration.self_application].canonical_type;
    let projection = ConstructorProjection::new(
        entities,
        &constructor.parameters,
        constructor.context_parameter.as_ref(),
        &declaration.type_params,
        owner_type,
        constructor.origin,
    )?;
    let kind = match &constructor.kind {
        ClassConstructorKind::Primary {
            base,
            primary_stores,
            ..
        } => Kind::ClassPrimary {
            base: base_delegation(&projection, base)?,
            primary_stores: primary_stores
                .iter()
                .map(|store| {
                    let index = constructor
                        .parameters
                        .iter()
                        .position(|parameter| parameter.id == store.parameter)
                        .ok_or(Error::Initialization(
                            crate::GenericInitializationBuildError::PrimaryStore,
                        ))?;
                    Ok(ExportPrimaryFieldStoreV1 {
                        field: class_field(&projection, store.field)?,
                        parameter: LocalValueSelector::Parameter {
                            declaration_index: u32::try_from(index)
                                .expect("constructor parameter index fits the source interface"),
                        },
                    })
                })
                .collect::<Result<_, Error>>()?,
        },
        ClassConstructorKind::Secondary { delegation, body } => match delegation {
            ClassSecondaryDelegation::This { target, arguments } => Kind::ClassSecondaryThis {
                delegation: ExportConstructorDelegationV1 {
                    target: entities
                        .class_constructor(*target, &projection.binders)
                        .map_err(Error::Entity)?,
                    arguments: projection.arguments(arguments)?,
                },
                body: projection.body(body)?,
            },
            ClassSecondaryDelegation::Terminal { base, .. } => Kind::ClassSecondaryTerminal {
                base: base_delegation(&projection, base)?,
                body: projection.body(body)?,
            },
        },
    };
    let effects = crate::production::callable_interfaces::source_constructor_effects(
        constructor.safety,
        crate::GcEffect::Managed,
        &crate::ReleaseCallability::Unavailable,
        &projection.binders,
    )
    .map_err(Error::Effects)?;
    ExportConstructorInitializationV1::try_new(
        DefaultConstructorRefV1::Class {
            declaration: entities.class_constructor_id(id).map_err(Error::Entity)?,
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

fn base_delegation(
    projection: &ConstructorProjection<'_, '_>,
    base: &BaseInitialization,
) -> Result<Option<ExportConstructorDelegationV1>, Error> {
    let BaseInitialization::Super { target, arguments } = base else {
        return Ok(None);
    };
    let target = match target {
        BaseInitializerTarget::Local(target) => projection
            .entities
            .class_constructor(*target, &projection.binders)
            .map_err(Error::Entity)?,
        BaseInitializerTarget::Imported { owner, callable } => {
            let selected = projection
                .entities
                .imported_dependency_source(*callable)
                .map_err(Error::Entity)?;
            let CallableTemplateOrigin::Constructor(declaration) =
                selected.interface().declaration()
            else {
                return Err(Error::Initialization(
                    crate::GenericInitializationBuildError::ConstructorKind,
                ));
            };
            DefaultConstructorRefV1::Class {
                declaration: DefaultClassConstructorIdV1::Source(declaration),
                owner_type: projection
                    .entities
                    .type_key(*owner, &projection.binders)
                    .map_err(Error::Entity)?,
            }
        }
    };
    Ok(Some(ExportConstructorDelegationV1 {
        target,
        arguments: projection.arguments(arguments)?,
    }))
}
