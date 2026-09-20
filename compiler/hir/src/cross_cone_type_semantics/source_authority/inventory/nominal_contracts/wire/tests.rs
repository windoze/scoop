use super::*;
use crate::cross_cone_type_semantics::protected_interfaces::tests::support::Fixture;
use crate::*;
use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, SourceDeclarationKey, SourceNominalKind,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn fixture() -> (Fixture, CanonicalNominalSourceContractsV1) {
    let mut fixture = Fixture::default();
    let owner = fixture.class("Owner");
    let child = fixture
        .graph
        .add("Child", SourceNominalKind::Class, &[owner]);
    let sibling = fixture
        .graph
        .add("Sibling", SourceNominalKind::Struct, &[owner]);
    let CallableTemplateOrigin::Function(function) =
        fixture.function(owner, "method", false, vec![])
    else {
        panic!()
    };
    let CallableTemplateOrigin::GenericFunction(generic) =
        fixture.function(owner, "generic", true, vec![])
    else {
        panic!()
    };
    let constructor = fixture.constructor(owner);
    let record = NominalSourceContractV1::try_new(
        owner.source,
        NominalInheritanceModalityV1::Open,
        CanonicalBinderListV1::try_new(vec![]).unwrap(),
        CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
        CanonicalPersistentIdsV1::try_new(vec![constructor]).unwrap(),
        CanonicalNestedMemberRefsV1::try_new(vec![
            NestedSourceMemberRefV1::GenericFunction(generic),
            NestedSourceMemberRefV1::Function(function),
        ])
        .unwrap(),
        CanonicalNestedNominalRefsV1::try_new(vec![child.source, sibling.source]).unwrap(),
        NominalSourceShapeV1::Class,
    )
    .unwrap();
    let table = CanonicalNominalSourceContractsV1::try_new(vec![record], &mut meter()).unwrap();
    (fixture, table)
}
fn decoded(table: &CanonicalNominalSourceContractsV1) -> DecodedCanonicalNominalSourceContractsV1 {
    let bytes = encode(table).unwrap();
    let decoded = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    decoded
}

#[test]
fn nominal_contract_wire_retains_all_eight_independent_fields() {
    let (mut fixture, table) = fixture();
    let record = &table.records()[0];
    let expected = [
        vec![0xa8, 1],
        encode(&record.owner()).unwrap(),
        vec![2],
        encode(&record.modality()).unwrap(),
        vec![3],
        encode(record.type_parameters()).unwrap(),
        vec![4],
        encode(record.supertypes()).unwrap(),
        vec![5],
        encode(record.constructors()).unwrap(),
        vec![6],
        encode(record.members()).unwrap(),
        vec![7],
        encode(record.children()).unwrap(),
        vec![8],
        encode(record.source_shape()).unwrap(),
    ]
    .concat();
    assert_eq!(encode(record).unwrap(), expected);
    assert_eq!(
        decoded(&table).resolve(&mut fixture, &mut meter()).unwrap(),
        table
    );
    assert_eq!(table.get(record.owner()), Some(record));
    assert_eq!(
        encode(&CanonicalNominalSourceContractsV1::default()).unwrap(),
        [0x80]
    );
}

#[test]
fn nominal_contract_wire_rejects_duplicate_and_reversed_references() {
    let (mut fixture, table) = fixture();
    for mutation in 0..5 {
        let mut decoded = decoded(&table);
        let record = &mut decoded.records[0];
        match mutation {
            0 => record.constructors.push(record.constructors[0]),
            1 => record.members.push(record.members[0].clone()),
            2 => record.members.reverse(),
            3 => record.children.push(record.children[0]),
            4 => record.children.reverse(),
            _ => unreachable!(),
        }
        let bytes = encode(&decoded).unwrap();
        let decoded: DecodedCanonicalNominalSourceContractsV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert!(
            decoded.resolve(&mut fixture, &mut meter()).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn nominal_contract_wire_rejects_duplicate_and_reversed_owners() {
    let (mut fixture, table) = fixture();
    let mut decoded = decoded(&table);
    decoded.records.push(decoded.records[0].clone());
    assert!(matches!(
        decoded.clone().resolve(&mut fixture, &mut meter()),
        Err(SourceInventoryError::NonCanonicalOrder { .. })
    ));
    let other = fixture.class("Other");
    decoded.records[1].owner =
        decode_canonical(&encode(&other.source).unwrap(), DecodeLimits::default()).unwrap();
    let mut ordered = decoded.clone().resolve(&mut fixture, &mut meter());
    if ordered.is_err() {
        decoded.records.reverse();
        ordered = decoded.clone().resolve(&mut fixture, &mut meter());
    }
    assert!(ordered.is_ok());
    decoded.records.reverse();
    assert!(matches!(
        decoded.resolve(&mut fixture, &mut meter()),
        Err(SourceInventoryError::NonCanonicalOrder { .. })
    ));
    let record = table.records()[0].clone();
    assert!(
        CanonicalNominalSourceContractsV1::try_new(vec![record.clone(), record], &mut meter())
            .is_err()
    );
}

#[test]
fn nominal_contract_wire_rejects_unknown_typed_identities() {
    let (mut fixture, table) = fixture();
    let key = SourceDeclarationKey::nominal(
        crate::cross_cone_type_semantics::inheritance::tests::support::site(&[]),
        CanonicalIdentifier::new("Absent").unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let unknown = SourceNominalId::from_source_declaration(&key).unwrap();
    let mut value = decoded(&table);
    value.records[0].owner =
        decode_canonical(&encode(&unknown).unwrap(), DecodeLimits::default()).unwrap();
    assert!(matches!(
        value.resolve(&mut fixture, &mut meter()),
        Err(SourceInventoryError::Reference(_))
    ));
}

#[test]
fn nominal_contract_wire_checks_modality_kind_and_binder_consistency() {
    let (mut fixture, table) = fixture();
    let mut value = decoded(&table);
    value.records[0].modality = NominalInheritanceModalityV1::Interface;
    assert!(value.resolve(&mut fixture, &mut meter()).is_err());
    let mut value = decoded(&table);
    value.records[0].source_shape = decode_canonical(
        &encode(&NominalSourceShapeV1::Interface).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    value.records[0].modality = NominalInheritanceModalityV1::Interface;
    assert!(value.resolve(&mut fixture, &mut meter()).is_err());
    let binders = CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
        CanonicalIdentifier::new("T").unwrap(),
        TypeParameterBoundsV1::Unconstrained,
    )])
    .unwrap();
    let mut value = decoded(&table);
    value.records[0].type_parameters =
        decode_canonical(&encode(&binders).unwrap(), DecodeLimits::default()).unwrap();
    assert!(value.resolve(&mut fixture, &mut meter()).is_err());
}

#[test]
fn nominal_contract_wire_uses_shared_resource_limits() {
    let (mut fixture, table) = fixture();
    for limits in [
        DecodeLimits {
            semantic_table_entries: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_recursion: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            decoded_nodes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            owned_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            decoded(&table).resolve(&mut fixture, &mut BudgetMeter::new(limits)),
            Err(SourceInventoryError::Resource(_))
        ));
    }
}

#[test]
fn nominal_contract_wire_rejects_inexact_products_and_unknown_tags() {
    let (_, table) = fixture();
    let bytes = encode(&table).unwrap();
    for size in [0xa7, 0xa9] {
        let mut corrupt = bytes.clone();
        corrupt[1] = size;
        assert!(
            decode_canonical::<DecodedCanonicalNominalSourceContractsV1>(
                &corrupt,
                DecodeLimits::default()
            )
            .is_err()
        );
    }
    let mut corrupt = bytes;
    let modality_index = 4 + encode(&table.records()[0].owner()).unwrap().len();
    corrupt[modality_index] = 0x17;
    assert!(
        decode_canonical::<DecodedCanonicalNominalSourceContractsV1>(
            &corrupt,
            DecodeLimits::default()
        )
        .is_err()
    );
}
