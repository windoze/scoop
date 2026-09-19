use super::*;
use crate::cross_cone_type_semantics::inheritance::tests::support::{Node, site};
use crate::cross_cone_type_semantics::protected_interfaces::tests::support::{Fixture, nominal};
use crate::*;
use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey, SourceNominalKind};
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};
mod authority;
mod generic;
mod generic_properties;
mod properties;
pub(in crate::cross_cone_type_semantics::protected_interfaces) mod support;
mod variants;
mod wire_tests;
use support::*;
fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn protected_nested_closes_recursive_children_constructors_and_private_members() {
    let mut fixture = Fixture::default();
    let outer = fixture.class("Outer");
    let owner = nested_class(&mut fixture, outer, "Nested");
    let child = fixture
        .graph
        .add("Child", SourceNominalKind::Class, &[outer, owner]);
    fixture
        .graph
        .visibility(child, DeclaredVisibilityV1::Private);
    fixture.graph.representations.insert(
        nominal(child),
        NominalRepresentationSupportV1::try_new(
            &fixture.graph.keys[&child.source],
            fixture.graph.access[&child.source].clone(),
            NominalRepresentationShapeV1::Class {
                base: scoop_identity::OptionalSignatureType::Absent,
                declared_fields: vec![],
            },
        )
        .unwrap(),
    );
    let child_source = source(
        NominalInheritanceModalityV1::Open,
        vec![],
        vec![],
        vec![],
        vec![],
    );
    let child_payload = payload(&mut fixture, child, child_source);
    let child_record = NominalSupportNestedInterfaceV1::try_new(
        child.source,
        fixture.graph.access[&child.source].clone(),
        child_payload,
    )
    .unwrap();
    let (public_ref, public) = function(
        &mut fixture,
        owner,
        "publicMethod",
        DeclaredVisibilityV1::Public,
    );
    let (private_ref, private) =
        function(&mut fixture, owner, "helper", DeclaredVisibilityV1::Private);
    let constructor = fixture.constructor(owner);
    let ctor = NominalSupportConstructorInterfaceV1::try_new(
        constructor,
        fixture.access(owner, DeclaredVisibilityV1::Public),
        fixture
            .payload(
                owner,
                CallableTemplateOrigin::Constructor(constructor),
                vec![],
                SignatureTypeKey::Nominal(nominal(owner)),
            )
            .source_signature,
    )
    .unwrap();
    let interface = source(
        NominalInheritanceModalityV1::Open,
        vec![constructor],
        vec![public_ref, private_ref],
        vec![child.source],
        vec![
            public,
            private,
            NestedSourceSupportV1::Constructor(Box::new(ctor)),
            NestedSourceSupportV1::NestedNominal(Box::new(child_record)),
        ],
    );
    let payload = payload(&mut fixture, owner, interface);
    let record = ProtectedNestedNominalInterfaceV1::try_new(
        owner.source,
        fixture.graph.access[&owner.source].clone(),
        payload,
    )
    .unwrap();
    let bytes = encode(&record).unwrap();
    let decoded: DecodedProtectedNestedNominalInterfaceV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(decoded.resolve(&mut fixture, &mut meter()).unwrap(), record);
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    let table = representations(&fixture);
    let checked = record
        .validate_source(&graph, &table, &mut fixture, &mut meter())
        .unwrap();
    assert_eq!(checked.record().declaration(), owner.source);
    let short = DecodeLimits {
        semantic_recursion: 1,
        ..DecodeLimits::default()
    };
    let error = record
        .validate_source(&graph, &table, &mut fixture, &mut BudgetMeter::new(short))
        .unwrap_err();
    assert!(format!("{error:?}").contains("Resource("));
}

#[test]
fn source_inventory_rejects_self_consistent_omission_and_wrong_concrete_join() {
    let mut fixture = Fixture::default();
    let outer = fixture.class("Outer");
    let owner = nested_class(&mut fixture, outer, "Nested");
    let (reference, callable) =
        function(&mut fixture, owner, "method", DeclaredVisibilityV1::Public);
    let interface = source(
        NominalInheritanceModalityV1::Open,
        vec![],
        vec![reference],
        vec![],
        vec![callable],
    );
    let expected = payload(&mut fixture, owner, interface);
    let omitted = ProtectedNestedNominalPayloadV1::try_new(
        owner.source,
        source(
            NominalInheritanceModalityV1::Open,
            vec![],
            vec![],
            vec![],
            vec![],
        ),
        expected.support(),
    )
    .unwrap();
    let record = NominalSupportNestedInterfaceV1::try_new(
        owner.source,
        fixture.graph.access[&owner.source].clone(),
        omitted,
    )
    .unwrap();
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    let table = representations(&fixture);
    assert!(matches!(
        record.validate_source(&graph, &table, &mut fixture, &mut meter()),
        Err(NestedSourceSemanticError::Inventory)
    ));
    let wrong = ProtectedNestedNominalPayloadV1::try_new(
        owner.source,
        expected.source_interface().clone(),
        NestedNominalSupportV1::ParamFree {
            inheritance_exact: outer.exact,
            representation_owner: nominal(owner),
        },
    )
    .unwrap();
    let record = NominalSupportNestedInterfaceV1::try_new(
        owner.source,
        fixture.graph.access[&owner.source].clone(),
        wrong,
    )
    .unwrap();
    assert!(matches!(
        record.validate_source(&graph, &table, &mut fixture, &mut meter()),
        Err(NestedSourceSemanticError::ConcreteSupport)
    ));
}

#[test]
fn nested_builder_rejects_missing_orphan_and_foreign_owner_support() {
    let mut fixture = Fixture::default();
    let outer = fixture.class("Outer");
    let owner = nested_class(&mut fixture, outer, "Nested");
    let (reference, callable) =
        function(&mut fixture, owner, "method", DeclaredVisibilityV1::Public);
    let support = NestedNominalSupportV1::ParamFree {
        inheritance_exact: owner.exact,
        representation_owner: nominal(owner),
    };
    for (refs, records) in [(vec![reference], vec![]), (vec![], vec![callable.clone()])] {
        assert!(matches!(
            ProtectedNestedNominalPayloadV1::try_new(
                owner.source,
                source(
                    NominalInheritanceModalityV1::Open,
                    vec![],
                    refs,
                    vec![],
                    records
                ),
                support
            ),
            Err(NestedSourceBuildError::ReferenceClosure)
        ));
    }
    let (foreign_ref, foreign) =
        function(&mut fixture, outer, "foreign", DeclaredVisibilityV1::Public);
    assert!(matches!(
        ProtectedNestedNominalPayloadV1::try_new(
            owner.source,
            source(
                NominalInheritanceModalityV1::Open,
                vec![],
                vec![foreign_ref],
                vec![],
                vec![foreign]
            ),
            support
        ),
        Err(NestedSourceBuildError::Owner)
    ));
    assert!(matches!(
        CanonicalNestedSourceSupportV1::try_new(vec![callable.clone(), callable]),
        Err(NestedSourceBuildError::Duplicate)
    ));
}
