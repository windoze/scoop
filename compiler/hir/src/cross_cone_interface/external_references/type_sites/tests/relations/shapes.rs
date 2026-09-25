use super::*;

#[test]
fn shape_roots_require_actual_operations_on_the_full_nominal_exact() {
    let types = Types::new();
    for role in [
        HirExpressionTypeRoleV1::Value,
        HirExpressionTypeRoleV1::SizeOf,
        HirExpressionTypeRoleV1::AlignOf,
        HirExpressionTypeRoleV1::TypeTest,
        HirExpressionTypeRoleV1::ArrayElement,
        HirExpressionTypeRoleV1::BoxedValue,
    ] {
        let table =
            types.table(vec![types.reference(
                types.unit,
                vec![types.occurrence(types.fixture.unit, role)],
            )]);
        types.validate(&table).unwrap();
        let roots = table
            .materialized_shape_dependencies(types.fixture.current, &types.graph)
            .unwrap();
        let expected = if matches!(
            role,
            HirExpressionTypeRoleV1::TypeTest | HirExpressionTypeRoleV1::BoxedValue
        ) {
            vec![(ConeIdentity::CORE, types.unit)]
        } else {
            vec![]
        };
        assert_eq!(roots, expected);
    }
    for role in [
        HirExpressionTypeRoleV1::TypeTest,
        HirExpressionTypeRoleV1::BoxedValue,
    ] {
        let occurrence = types.occurrence(types.tuple, role);
        let table = types.table(vec![
            types.reference(types.unit, vec![occurrence.clone()]),
            types.reference(types.any, vec![occurrence]),
        ]);
        types.validate(&table).unwrap();
        assert!(
            table
                .materialized_shape_dependencies(types.fixture.current, &types.graph)
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn repeated_operations_share_one_dependency_but_do_not_erase_type_sites() {
    let types = Types::new();
    let table = types.table(vec![types.reference(
        types.unit,
        vec![
            site(&types.fixture, 0, HirExpressionTypeRoleV1::TypeTest),
            site(&types.fixture, 1, HirExpressionTypeRoleV1::BoxedValue),
            site(&types.fixture, 2, HirExpressionTypeRoleV1::TypeTest),
        ],
    )]);
    types.validate(&table).unwrap();
    assert_eq!(table.records()[0].type_sites().records().len(), 3);
    let query = || table.materialized_shape_dependencies(types.fixture.current, &types.graph);

    assert_eq!(query().unwrap(), vec![(ConeIdentity::CORE, types.unit)]);

    query().unwrap();
}

#[test]
fn shape_roots_reject_wrong_nominal_provider_and_current_owner() {
    let types = Types::new();
    let occurrence = types.occurrence(types.fixture.unit, HirExpressionTypeRoleV1::BoxedValue);
    let wrong_type = types.table(vec![types.reference(types.any, vec![occurrence.clone()])]);
    assert!(matches!(
        wrong_type.materialized_shape_dependencies(types.fixture.current, &types.graph),
        Err(Error::Target(_))
    ));
    let reference = types.reference(types.unit, vec![occurrence]);
    let wrong_provider = ExternalHirReferenceV1::try_new(
        types.fixture.provider,
        reference.target(),
        reference.roles().clone(),
        reference.witnesses().clone(),
        reference.call_sites().clone(),
        reference.type_sites().clone(),
    )
    .unwrap();
    assert!(matches!(
        types
            .table(vec![wrong_provider])
            .materialized_shape_dependencies(types.fixture.current, &types.graph),
        Err(Error::Target(_))
    ));
    assert!(matches!(
        types
            .table(vec![reference])
            .materialized_shape_dependencies(ConeIdentity::CORE, &types.graph),
        Err(Error::Target(_))
    ));
}
