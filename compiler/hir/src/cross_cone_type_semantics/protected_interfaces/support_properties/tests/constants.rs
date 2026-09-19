use super::*;
use crate::cross_cone_type_semantics::inheritance::tests::support::site;
use scoop_identity::{CanonicalIdentifier, SourceDeclarationKey};

fn fixture() -> (Fixture, NominalSupportPropertyInterfaceV1) {
    let mut fixture = Fixture::default();
    let outer = fixture.class("Outer");
    let owner = fixture
        .graph
        .add("Singleton", SourceNominalKind::Object, &[outer]);
    let access = DeclarationAccessSourceV1::try_new(
        DeclaredVisibilityV1::Protected,
        vec![outer.source],
        fixture.graph.origins[&owner.source].clone(),
    )
    .unwrap();
    fixture.graph.access.insert(owner.source, access.clone());
    let old = fixture.graph.representations[&nominal(owner)]
        .shape()
        .clone();
    fixture.graph.representations.insert(
        nominal(owner),
        NominalRepresentationSupportV1::try_new(&fixture.graph.keys[&owner.source], access, old)
            .unwrap(),
    );
    let boolean = fixture.graph.add("Boolean", SourceNominalKind::Struct, &[]);
    let key = SourceDeclarationKey::property(
        site(&fixture.owner_chain(owner)),
        CanonicalIdentifier::new("enabled").unwrap(),
    );
    let property = PersistentPropertyId::from_source_declaration(&key).unwrap();
    let access = fixture.access(owner, DeclaredVisibilityV1::Public);
    fixture.const_sources.insert(
        property,
        ConstPropertyDeclarationSourceV1::new(key, access.definition_origin().origin().clone()),
    );
    fixture
        .const_types
        .insert(CanonicalConstValueKindV1::Boolean, nominal(boolean));
    fixture
        .property_types
        .insert(property, SignatureTypeKey::Nominal(nominal(boolean)));
    let value = ExportConstValueV1::new(
        property,
        SignatureTypeKey::Nominal(nominal(boolean)),
        CanonicalConstValueV1::Boolean(CanonicalBooleanV1::True),
        access.definition_origin().clone(),
    );
    let record = NominalSupportPropertyInterfaceV1::try_new(
        property,
        access,
        NominalSupportPropertyPayloadV1::Const { value },
    )
    .unwrap();
    (fixture, record)
}

#[test]
fn nested_object_const_has_a_typed_value_without_an_accessor_or_public_property_record() {
    let (mut fixture, record) = fixture();
    assert!(fixture.accessors.is_empty());
    let bytes = encode(&record).unwrap();
    let decoded: DecodedNominalSupportPropertyInterfaceV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(
        decoded.clone().resolve(&mut fixture, &mut meter()).unwrap(),
        record
    );
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    let CheckedNominalSupportPropertySourceV1::Const(checked) = record
        .validate_source(&graph, &mut fixture, &mut meter())
        .unwrap()
    else {
        panic!("const support required")
    };
    assert_eq!(
        checked.value().value(),
        &CanonicalConstValueV1::Boolean(CanonicalBooleanV1::True)
    );
    assert!(
        !graph
            .replay_declaration_access(checked.declaration_access(), &mut meter())
            .unwrap()
            .lookup()
            .domain()
            .is_universal()
    );
    assert!(
        decoded
            .resolve(
                &mut fixture,
                &mut BudgetMeter::new(DecodeLimits {
                    logical_heap_bytes: 0,
                    ..DecodeLimits::default()
                })
            )
            .is_err()
    );
}

#[test]
fn const_support_rejects_a_type_kind_mismatch_and_mismatched_outer_identity() {
    let (mut fixture, mut record) = fixture();
    let NominalSupportPropertyPayloadV1::Const { value } = record.payload() else {
        unreachable!()
    };
    let bad = ExportConstValueV1::new(
        value.property(),
        SignatureTypeKey::Nominal(nominal(fixture.unit)),
        value.value().clone(),
        value.definition_origin().clone(),
    );
    record.payload = NominalSupportPropertyPayloadV1::Const { value: bad };
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    assert!(matches!(
        record.validate_source(&graph, &mut fixture, &mut meter()),
        Err(NominalSupportPropertySemanticError::ConstType)
    ));
    let key = SourceDeclarationKey::property(
        site(&[record.owner()]),
        CanonicalIdentifier::new("other").unwrap(),
    );
    let other = PersistentPropertyId::from_source_declaration(&key).unwrap();
    assert!(matches!(
        NominalSupportPropertyInterfaceV1::try_new(
            other,
            record.declaration_access().clone(),
            record.payload().clone()
        ),
        Err(NominalSupportPropertyBuildError::ConstIdentity)
    ));
    assert!(
        decode_canonical::<DecodedNominalSupportPropertyPayloadV1>(
            &[0xa2, 0, 3, 1, 0],
            DecodeLimits::default()
        )
        .is_err()
    );
}
