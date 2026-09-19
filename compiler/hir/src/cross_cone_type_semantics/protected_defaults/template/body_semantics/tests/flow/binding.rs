use super::*;

#[test]
fn protected_struct_binding_callback_keeps_the_actual_source_and_shared_budget() {
    let f = Fixture::new();
    let selectors: [LocalValueSelector; 6] =
        std::array::from_fn(|index| LocalValueSelector::Synthetic {
            path: path(StructuralDefinitionSiteRole::SyntheticValue, index as u32),
            role: SyntheticLocalRole::Temporary,
        });
    let temporary =
        |index: usize| DefaultBindingTemporaryV1::new(selectors[index].clone(), value(&f));
    let leaf_selector = LocalValueSelector::LocalDeclaration {
        path: path(StructuralDefinitionSiteRole::LocalDeclaration, 7),
    };
    let leaf =
        DefaultBindingLeafV1::new(leaf_selector.clone(), value(&f), CanonicalBooleanV1::False);
    let shape = DefaultBindingShapeV1::try_struct(
        value(&f),
        vec![DefaultBindingStructFieldV1::new(
            7,
            DefaultBindingShapeV1::binding(leaf.clone()),
        )],
    )
    .unwrap();
    let binding = DefaultBindingPlanV1::try_new(
        temporary(4),
        shape,
        vec![
            DefaultBindingActionV1::project(
                temporary(4),
                temporary(5),
                DefaultBindingProjectionV1::struct_field(f.field, value(&f)),
                f.origin(),
            ),
            DefaultBindingActionV1::bind(temporary(5), leaf, f.origin()),
        ],
    )
    .unwrap();
    let conformance =
        DefaultIteratorConformanceV1::new(temporary(1), temporary(2), value(&f), f.origin());
    let option = DefaultAppliedOptionV1::new(
        DefaultEnumVariantFieldRefV1::new(f.variant_field, value(&f)),
        DefaultEnumVariantRefV1::new(f.variant, value(&f)),
    );
    let next =
        DefaultIteratorNextV1::new(f.callable(), temporary(3), option, temporary(4), f.origin());
    let plan = DefaultForIterationPlanV1::try_new(
        vec![],
        temporary(0),
        unit(&f),
        vec![],
        unit(&f),
        conformance,
        next,
        binding,
        vec![],
    )
    .unwrap();
    let mut locals: Vec<_> = selectors
        .into_iter()
        .map(|selector| local(&f, selector))
        .collect();
    locals.push(local(&f, leaf_selector));
    let template = template(
        &f,
        locals,
        vec![],
        vec![statement(&f, DefaultStatementKindV1::For(Box::new(plan)))],
        unit(&f),
    );
    let mut resources = meter();
    let path = WirePath::root();
    let mut authority = FlowAuthority {
        observation: Observation::new(&template, &resources, &path),
        field: f.field,
    };
    template
        .validate_local_data_flow_semantics(&mut authority, &mut resources, &path)
        .unwrap();
    assert_eq!(authority.observation.calls, 1);
    assert!(resources.usage().validation_work_units >= 13);
    let mut limited = BudgetMeter::new(DecodeLimits {
        validation_work_units: 0,
        ..DecodeLimits::default()
    });
    assert!(matches!(
        template.validate_local_data_flow_semantics(&mut authority, &mut limited, &path),
        Err(ExportDefaultLocalDataFlowValidationError::Resource(_))
    ));
}
