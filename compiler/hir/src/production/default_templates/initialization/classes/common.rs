use super::*;

pub(in crate::production::default_templates) fn project(
    entities: &DefaultEntityProjector<'_>,
    class: crate::ClassId,
) -> Result<Vec<ExportCommonInitializationStepV1>, Error> {
    let inputs = entities.inputs();
    let declaration = &inputs.type_inputs.classes[class];
    let owner_type = inputs.class_applications[declaration.self_application].canonical_type;
    let primary = declaration.constructors.iter().copied().find(|&id| {
        matches!(
            inputs.class_constructors[id].kind,
            ClassConstructorKind::Primary { .. }
        )
    });
    let context_constructor = primary.or_else(|| declaration.constructors.first().copied());
    let common_source =
        declaration
            .constructors
            .iter()
            .find_map(|&id| match &inputs.class_constructors[id].kind {
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
    let (Some(context), Some(common)) = (context_constructor, common_source) else {
        return Ok(Vec::new());
    };
    let constructor = &inputs.class_constructors[context];
    let parameters = if primary.is_some() {
        constructor.parameters.as_slice()
    } else {
        &[]
    };
    let projection = ConstructorProjection::new(
        entities,
        parameters,
        constructor.context_parameter.as_ref(),
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
        .collect()
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
