use super::*;
use crate::{
    BaseInitialization, BaseInitializerTarget, ClassConstructorKind, ClassInitializationStep,
    ClassSecondaryDelegation, DefaultClassConstructorIdV1, DefaultConstructorRefV1,
    ExportCommonInitializationStepV1, ExportConstructorDelegationV1,
    ExportConstructorInitializationKindV1 as Kind, ExportConstructorInitializationV1,
    ExportPrimaryFieldStoreV1,
};
use scoop_identity::{CallableTemplateOrigin, LocalValueSelector};

pub(super) fn project(
    entities: &DefaultEntityProjector<'_>,
    owner: scoop_identity::PersistentGenericTypeId,
    class: crate::ClassId,
) -> Result<ExportGenericNominalInitializationV1, Error> {
    let export = entities.export();
    let declaration = &export.classes[class];
    let owner_type = export.class_applications[declaration.self_application].canonical_type;
    let primary = declaration.constructors.iter().copied().find(|&id| {
        matches!(
            export.class_constructors[id].kind,
            ClassConstructorKind::Primary { .. }
        )
    });
    let context_constructor = primary.or_else(|| declaration.constructors.first().copied());
    let common_source =
        declaration
            .constructors
            .iter()
            .find_map(|&id| match &export.class_constructors[id].kind {
                ClassConstructorKind::Primary {
                    common_initialization,
                    ..
                }
                | ClassConstructorKind::Secondary {
                    delegation:
                        ClassSecondaryDelegation::Terminal {
                            common_initialization,
                            ..
                        },
                    ..
                } => Some(common_initialization),
                ClassConstructorKind::Secondary {
                    delegation: ClassSecondaryDelegation::This { .. },
                    ..
                } => None,
            });
    let common = if let (Some(context), Some(common)) = (context_constructor, common_source) {
        let constructor = &export.class_constructors[context];
        let parameters = if primary.is_some() {
            constructor.parameters.as_slice()
        } else {
            &[]
        };
        let projection = ConstructorProjection::new(
            entities,
            parameters,
            &declaration.type_params,
            owner_type,
            crate::DefinitionOrigin {
                context: constructor.evaluation_context,
                ..constructor.origin
            },
        )?;
        common
            .iter()
            .map(|step| common_step(&projection, step))
            .collect::<Result<_, _>>()?
    } else {
        Vec::new()
    };
    let constructors = declaration
        .constructors
        .iter()
        .map(|&id| {
            let constructor = &export.class_constructors[id];
            let projection = ConstructorProjection::new(
                entities,
                &constructor.parameters,
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
                                    declaration_index: u32::try_from(index).expect(
                                        "constructor parameter index fits the source interface",
                                    ),
                                },
                            })
                        })
                        .collect::<Result<_, Error>>()?,
                },
                ClassConstructorKind::Secondary { delegation, body } => match delegation {
                    ClassSecondaryDelegation::This { target, arguments } => {
                        Kind::ClassSecondaryThis {
                            delegation: ExportConstructorDelegationV1 {
                                target: entities
                                    .class_constructor(*target, &projection.binders)
                                    .map_err(Error::Entity)?,
                                arguments: projection.arguments(arguments)?,
                            },
                            body: projection.body(body)?,
                        }
                    }
                    ClassSecondaryDelegation::Terminal { base, .. } => {
                        Kind::ClassSecondaryTerminal {
                            base: base_delegation(&projection, base)?,
                            body: projection.body(body)?,
                        }
                    }
                },
            };
            let effects = crate::production::callable_interfaces::source_constructor_effects(
                constructor.safety,
                crate::GcEffect::Managed,
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
        })
        .collect::<Result<_, Error>>()?;
    ExportGenericNominalInitializationV1::try_new(owner, common, constructors)
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

fn class_field(
    projection: &ConstructorProjection<'_, '_>,
    field: crate::InitializingClassFieldRef,
) -> Result<crate::DefaultFieldRefV1, Error> {
    projection
        .entities
        .field(
            crate::FieldRef::ClassField {
                owner: field.owner,
                field: field.field,
            },
            &projection.binders,
        )
        .map_err(Error::Entity)
}

fn common_step(
    projection: &ConstructorProjection<'_, '_>,
    step: &ClassInitializationStep,
) -> Result<ExportCommonInitializationStepV1, Error> {
    match step {
        ClassInitializationStep::Field {
            field, initializer, ..
        } => Ok(ExportCommonInitializationStepV1::Field {
            field: class_field(projection, *field)?,
            value: projection.fragment(
                &initializer.locals,
                &initializer.statements,
                std::slice::from_ref(&initializer.value),
            )?,
        }),
        ClassInitializationStep::InitBlock { body, .. } => projection
            .body(body)
            .map(ExportCommonInitializationStepV1::Body),
    }
}
