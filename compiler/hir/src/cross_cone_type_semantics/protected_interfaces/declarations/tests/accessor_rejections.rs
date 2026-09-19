use super::*;
use crate::cross_cone_type_semantics::protected_interfaces::property::tests::support::setup;

#[test]
fn declaration_source_table_rejects_a_missing_protected_setter_after_inventory_join() {
    let (mut fixture, _, property, getter, _) = setup(Some(DeclaredVisibilityV1::Protected));
    let table = with_inventory(
        &mut fixture,
        vec![
            ProtectedDeclarationInterfaceV1::Property(Box::new(property)),
            ProtectedDeclarationInterfaceV1::Callable(Box::new(getter)),
        ],
    );
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    let representations = representations(&fixture);
    assert!(matches!(
        table.validate_sources(&graph, &representations, &mut fixture, &mut meter()),
        Err(ProtectedDeclarationSemanticError::Table(
            ProtectedDeclarationTableError::AccessorClosure
        ))
    ));
}

#[test]
fn accessor_presence_cannot_substitute_for_its_source_signature_validation() {
    let (mut fixture, owner, property, getter, _) = setup(None);
    let forged = fixture.record(
        owner,
        getter.declaration(),
        fixture.payload(
            owner,
            getter.declaration(),
            vec![],
            SignatureTypeKey::Nominal(nominal(fixture.unit)),
        ),
    );
    let table = with_inventory(
        &mut fixture,
        vec![
            ProtectedDeclarationInterfaceV1::Property(Box::new(property)),
            ProtectedDeclarationInterfaceV1::Callable(Box::new(forged)),
        ],
    );
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    let representations = representations(&fixture);
    assert!(matches!(
        table.validate_sources(&graph, &representations, &mut fixture, &mut meter()),
        Err(ProtectedDeclarationSemanticError::Callable(
            ProtectedCallableSemanticError::Result
        ))
    ));
}
