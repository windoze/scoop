use super::*;
use crate::cross_cone_type_semantics::protected_interfaces::tests::support::{Fixture, nominal};
use crate::*;
use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey};
use scoop_wire::{decode_canonical, encode};

mod accessor_rejections;
mod support;
mod wire_tests;
use support::*;

impl ProtectedDeclarationSemanticAuthority<&'static str> for Fixture {
    fn required_protected_declarations(
        &self,
    ) -> Result<&CanonicalProtectedDeclarationRefsV1, &'static str> {
        Ok(&self.protected_roots)
    }
}

impl TypeSectionDeclarationSemanticAuthority<&'static str> for Fixture {}

#[test]
fn protected_declaration_table_roundtrips_all_four_kinds_and_closes_sources() {
    let (mut fixture, table) = complete();
    let bytes = encode(&table).unwrap();
    let decoded: DecodedCanonicalProtectedDeclarationInterfacesV1 =
        decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(decoded.resolve(&mut fixture).unwrap(), table);
    let roots = fixture.protected_roots.clone();
    let decoded: DecodedCanonicalProtectedDeclarationRefsV1 =
        decode_canonical(&encode(&roots).unwrap()).unwrap();
    assert_eq!(decoded.resolve(&mut fixture).unwrap(), roots);
    let graph_source = fixture.graph.clone();
    let graph =
        CheckedNominalInheritanceGraphV1::validate(graph_source.records.values(), &graph_source)
            .unwrap();
    let representations = representations(&fixture);
    assert_eq!(
        table
            .validate_sources(&graph, &representations, &mut fixture)
            .unwrap()
            .table(),
        &table
    );
    let keys = roots
        .values()
        .iter()
        .map(|key| encode(key).unwrap())
        .collect::<Vec<_>>();
    assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn source_table_requires_the_independent_complete_declaration_inventory() {
    let (mut fixture, table) = complete();
    let graph_source = fixture.graph.clone();
    let graph =
        CheckedNominalInheritanceGraphV1::validate(graph_source.records.values(), &graph_source)
            .unwrap();
    let representations = representations(&fixture);
    let missing =
        CanonicalProtectedDeclarationInterfacesV1::try_new(table.records()[1..].to_vec()).unwrap();
    assert!(matches!(
        missing.validate_sources(&graph, &representations, &mut fixture),
        Err(ProtectedDeclarationSemanticError::Table(
            ProtectedDeclarationTableError::Inventory
        ))
    ));
    fixture.protected_roots = CanonicalProtectedDeclarationRefsV1::default();
    assert!(matches!(
        table.validate_sources(&graph, &representations, &mut fixture),
        Err(ProtectedDeclarationSemanticError::Table(
            ProtectedDeclarationTableError::Inventory
        ))
    ));
}

#[test]
fn property_closure_requires_getter_and_only_truly_protected_setters() {
    use crate::cross_cone_type_semantics::protected_interfaces::property::tests::support::setup;
    for visibility in [
        None,
        Some(DeclaredVisibilityV1::Private),
        Some(DeclaredVisibilityV1::Protected),
    ] {
        let (mut fixture, _, property, getter, setter) = setup(visibility);
        let getter_ref = ProtectedDeclarationInterfaceV1::Callable(Box::new(getter));
        let property_ref = ProtectedDeclarationInterfaceV1::Property(Box::new(property));
        let mut records = vec![getter_ref, property_ref];
        if let Some(setter) = setter {
            records.push(ProtectedDeclarationInterfaceV1::Callable(Box::new(setter)));
        }
        let table = with_inventory(&mut fixture, records);
        let graph_source = fixture.graph.clone();
        let graph = CheckedNominalInheritanceGraphV1::validate(
            graph_source.records.values(),
            &graph_source,
        )
        .unwrap();
        let representations = representations(&fixture);
        table
            .validate_sources(&graph, &representations, &mut fixture)
            .unwrap();
        let missing = with_inventory(
            &mut fixture,
            table
                .records()
                .iter()
                .filter(|record| matches!(record, ProtectedDeclarationInterfaceV1::Property(_)))
                .cloned()
                .collect(),
        );
        assert!(matches!(
            missing.validate_sources(&graph, &representations, &mut fixture),
            Err(ProtectedDeclarationSemanticError::Table(
                ProtectedDeclarationTableError::AccessorClosure
            ))
        ));
    }
}

#[test]
fn protected_accessor_root_does_not_require_a_protected_property_record() {
    use crate::cross_cone_type_semantics::protected_interfaces::property::tests::support::setup;
    let (mut fixture, _, _, _, setter) = setup(Some(DeclaredVisibilityV1::Protected));
    let table = with_inventory(
        &mut fixture,
        vec![ProtectedDeclarationInterfaceV1::Callable(Box::new(
            setter.unwrap(),
        ))],
    );
    let graph_source = fixture.graph.clone();
    let graph =
        CheckedNominalInheritanceGraphV1::validate(graph_source.records.values(), &graph_source)
            .unwrap();
    let representations = representations(&fixture);
    table
        .validate_sources(&graph, &representations, &mut fixture)
        .unwrap();
}
