use super::*;
use crate::*;
use scoop_identity::{
    AccessorRole, CallableTemplateOrigin, DecodedPersistentId, PersistentIdResolver, PropertyOwner,
    SourceDeclarationKey,
};
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

mod rejections;
mod slots;
mod support;
mod wire;
use support::setup;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn protected_property_keeps_private_setter_restricted_and_joins_the_checked_getter() {
    let (mut fixture, _, property, getter, _) = setup(Some(DeclaredVisibilityV1::Private));
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    let mut getter_authority = fixture.clone();
    let checked_property = property
        .validate_source(&graph, &mut fixture, &mut meter())
        .unwrap();
    let checked_getter = getter
        .validate_source(&graph, &mut getter_authority, &mut meter())
        .unwrap();
    checked_property
        .validate_accessor_contracts(checked_getter, None, &mut meter())
        .unwrap();
    assert!(matches!(
        checked_property.validate_accessor_contracts(
            checked_getter,
            Some(checked_getter),
            &mut meter()
        ),
        Err(ProtectedPropertyAccessorClosureError::Setter)
    ));
    assert_eq!(
        checked_property.record().declaration(),
        property.declaration()
    );
}

#[test]
fn protected_setter_requires_its_own_checked_source_callable() {
    let (mut fixture, _, property, getter, setter) = setup(Some(DeclaredVisibilityV1::Protected));
    let setter = setter.unwrap();
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    let mut getter_authority = fixture.clone();
    let mut setter_authority = fixture.clone();
    let checked_property = property
        .validate_source(&graph, &mut fixture, &mut meter())
        .unwrap();
    let checked_getter = getter
        .validate_source(&graph, &mut getter_authority, &mut meter())
        .unwrap();
    let checked_setter = setter
        .validate_source(&graph, &mut setter_authority, &mut meter())
        .unwrap();
    checked_property
        .validate_accessor_contracts(checked_getter, Some(checked_setter), &mut meter())
        .unwrap();
    assert!(matches!(
        checked_property.validate_accessor_contracts(checked_getter, None, &mut meter()),
        Err(ProtectedPropertyAccessorClosureError::Setter)
    ));
    assert!(matches!(
        checked_property.validate_accessor_contracts(
            checked_setter,
            Some(checked_getter),
            &mut meter()
        ),
        Err(ProtectedPropertyAccessorClosureError::Getter)
    ));
}

#[test]
fn setter_domains_are_compared_semantically_instead_of_by_visibility_order() {
    let (mut fixture, _, property, _, _) = setup(Some(DeclaredVisibilityV1::Internal));
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    assert!(matches!(
        property.validate_source(&graph, &mut fixture, &mut meter()),
        Err(ProtectedPropertySemanticError::SetterDomain)
    ));
}
