use super::*;
use crate::*;
use scoop_identity::{AccessorRole, CallableTemplateOrigin, SignatureTypeKey};
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

mod rejections;
mod support;
mod wire;
use support::{Fixture, nominal};
fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn protected_generic_method_keeps_source_binders_without_gaining_a_dispatch_slot() {
    let mut fixture = Fixture::default();
    let owner = fixture.class("Owner");
    let binder = SignatureTypeKey::Binder { depth: 0, index: 0 };
    let declaration = fixture.function(owner, "method", true, vec![binder.clone()]);
    let payload = fixture.payload(owner, declaration, vec![binder.clone()], binder);
    let record = fixture.record(owner, declaration, payload.clone());
    assert!(matches!(
        record.payload().source_interface(),
        ProtectedSourceInterfaceUseV1::GenericFunction(_)
    ));
    assert!(record.payload().slot_relations().is_empty());
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    let checked = record
        .validate_source(&graph, &mut fixture, &mut meter())
        .unwrap();
    assert_eq!(checked.declaration(), declaration);
    assert_eq!(checked.payload().type_parameters().len_u32(), 1);
    let mut bad = payload;
    bad.modality = CallableModalityV1::Open;
    assert!(matches!(
        ProtectedCallableInterfaceV1::try_new(
            declaration,
            fixture.access(owner, DeclaredVisibilityV1::Protected),
            bad
        ),
        Err(ProtectedCallableInterfaceBuildError::Modality)
    ));
}

#[test]
fn protected_accessor_source_shapes_are_joined_to_the_logical_property() {
    let mut fixture = Fixture::default();
    let owner = fixture.class("Owner");
    let value = SignatureTypeKey::Nominal(nominal(owner));
    let unit = SignatureTypeKey::Nominal(nominal(fixture.unit));
    let getter = fixture.accessor(owner, AccessorRole::Getter, value.clone());
    let setter = fixture.accessor(owner, AccessorRole::Setter, value.clone());
    let get = fixture.record(
        owner,
        getter,
        fixture.payload(owner, getter, vec![], value.clone()),
    );
    let set_payload = fixture.payload(owner, setter, vec![value], unit.clone());
    let set = fixture.record(owner, setter, set_payload.clone());
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    get.validate_source(&graph, &mut fixture, &mut meter())
        .unwrap();
    set.validate_source(&graph, &mut fixture, &mut meter())
        .unwrap();
    assert_eq!(
        set.payload().source_interface(),
        ProtectedSourceInterfaceUseV1::AccessorNoSourceInterface
    );
    let bad_get = fixture.record(owner, getter, fixture.payload(owner, getter, vec![], unit));
    assert!(matches!(
        bad_get.validate_source(&graph, &mut fixture, &mut meter()),
        Err(ProtectedCallableSemanticError::Result)
    ));
    let mut bad_payload = set_payload;
    bad_payload.parameters = CanonicalSourceParameterShapesV1::try_new(vec![]).unwrap();
    let bad = fixture.record(owner, setter, bad_payload);
    assert!(matches!(
        bad.validate_source(&graph, &mut fixture, &mut meter()),
        Err(ProtectedCallableSemanticError::ParameterShape)
    ));
}

#[test]
fn protected_constructor_is_separate_and_preserves_the_source_result_owner() {
    let mut fixture = Fixture::default();
    let owner = fixture.class("Owner");
    let constructor = fixture.constructor(owner);
    let declaration = CallableTemplateOrigin::Constructor(constructor);
    let payload = fixture.payload(
        owner,
        declaration,
        vec![],
        SignatureTypeKey::Nominal(nominal(owner)),
    );
    let record = ProtectedConstructorInterfaceV1::try_new(
        constructor,
        fixture.access(owner, DeclaredVisibilityV1::Protected),
        payload.clone(),
    )
    .unwrap();
    assert!(matches!(
        ProtectedCallableInterfaceV1::try_new(
            declaration,
            fixture.access(owner, DeclaredVisibilityV1::Protected),
            payload.clone()
        ),
        Err(ProtectedCallableInterfaceBuildError::DeclarationKind)
    ));
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    record
        .validate_source(&graph, &mut fixture, &mut meter())
        .unwrap();
    let mut bad = payload;
    bad.result = SignatureTypeKey::Nominal(nominal(fixture.unit));
    let bad = ProtectedConstructorInterfaceV1::try_new(
        constructor,
        fixture.access(owner, DeclaredVisibilityV1::Protected),
        bad,
    )
    .unwrap();
    assert!(matches!(
        bad.validate_source(&graph, &mut fixture, &mut meter()),
        Err(ProtectedCallableSemanticError::Result)
    ));
    let decoded: DecodedProtectedConstructorInterfaceV1 =
        decode_canonical(&encode(&record).unwrap(), DecodeLimits::default()).unwrap();
    assert_eq!(decoded.resolve(&mut fixture, &mut meter()).unwrap(), record);
}
