use scoop_identity::{CoreBuiltinNominal, ExactTypeKey, SourceNominalKind};
use scoop_wire::{BudgetMeter, DecodeLimits, Encoder, WireEncode, decode_canonical, encode};

use super::*;
use crate::cross_cone_type_semantics::wire;

pub(in crate::cross_cone_type_semantics) mod support;
use support::Fixture;
mod objects;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn checked_graph_accepts_diamond_interface_closure_and_lexical_sources() {
    let mut fixture = Fixture::default();
    let root = fixture.add("Root", SourceNominalKind::Interface, &[]);
    let left = fixture.add("Left", SourceNominalKind::Interface, &[]);
    let right = fixture.add("Right", SourceNominalKind::Interface, &[]);
    fixture.edges(left, None, &[root]);
    fixture.edges(right, None, &[root]);
    let base = fixture.add("Base", SourceNominalKind::Class, &[]);
    let derived = fixture.add("Derived", SourceNominalKind::Class, &[]);
    fixture.edges(derived, Some(base), &[left, right]);
    let nested = fixture.add("Nested", SourceNominalKind::Struct, &[derived]);
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.records.values(),
        &fixture,
        &mut meter(),
    )
    .unwrap();
    assert!(
        graph
            .is_subclass(derived.exact, base.exact, &mut meter())
            .unwrap()
    );
    assert!(
        !graph
            .is_subclass(base.exact, derived.exact, &mut meter())
            .unwrap()
    );
    assert!(
        !graph
            .is_subclass(nested.exact, base.exact, &mut meter())
            .unwrap()
    );
    assert!(
        graph
            .lexically_contains(derived.source, nested.source)
            .unwrap()
    );
}

#[test]
fn class_and_interface_cycles_fail_independently() {
    for kind in [SourceNominalKind::Class, SourceNominalKind::Interface] {
        let mut fixture = Fixture::default();
        let left = fixture.add("Left", kind, &[]);
        let right = fixture.add("Right", kind, &[]);
        if kind == SourceNominalKind::Class {
            fixture.edges(left, Some(right), &[]);
            fixture.edges(right, Some(left), &[]);
        } else {
            fixture.edges(left, None, &[right]);
            fixture.edges(right, None, &[left]);
        }
        assert!(matches!(
            CheckedNominalInheritanceGraphV1::validate(
                fixture.records.values(),
                &fixture,
                &mut meter()
            ),
            Err(InheritanceGraphError::Cycle(_))
        ));
    }
}

#[test]
fn missing_wrong_kind_and_final_base_are_rejected() {
    let mut fixture = Fixture::default();
    let base = fixture.add("Base", SourceNominalKind::Class, &[]);
    let child = fixture.add("Child", SourceNominalKind::Class, &[]);
    let interface = fixture.add("Interface", SourceNominalKind::Interface, &[]);
    fixture.edges(child, Some(interface), &[]);
    assert!(matches!(
        CheckedNominalInheritanceGraphV1::validate(
            fixture.records.values(),
            &fixture,
            &mut meter()
        ),
        Err(InheritanceGraphError::ClassBaseKind { .. })
    ));
    fixture.edges(child, None, &[base]);
    assert!(matches!(
        CheckedNominalInheritanceGraphV1::validate(
            fixture.records.values(),
            &fixture,
            &mut meter()
        ),
        Err(InheritanceGraphError::InterfaceEdgeKind { .. })
    ));
    fixture.edges(child, Some(base), &[]);
    fixture.modality(base, NominalInheritanceModalityV1::Final);
    assert!(matches!(
        CheckedNominalInheritanceGraphV1::validate(
            fixture.records.values(),
            &fixture,
            &mut meter()
        ),
        Err(InheritanceGraphError::FinalBase { .. })
    ));
    fixture.records.remove(&base.exact);
    assert!(matches!(
        CheckedNominalInheritanceGraphV1::validate(
            fixture.records.values(),
            &fixture,
            &mut meter()
        ),
        Err(InheritanceGraphError::MissingNode(_))
    ));
}

#[test]
fn foundation_identity_source_and_modality_are_not_trusted_from_the_edges() {
    let mut fixture = Fixture::default();
    let value = fixture.add("Value", SourceNominalKind::Struct, &[]);
    fixture.modality(value, NominalInheritanceModalityV1::Open);
    assert!(matches!(
        CheckedNominalInheritanceGraphV1::validate(
            fixture.records.values(),
            &fixture,
            &mut meter()
        ),
        Err(InheritanceGraphError::KindModality(_))
    ));
    fixture.modality(value, NominalInheritanceModalityV1::Final);
    let old = fixture
        .exacts
        .insert(
            value.exact,
            ExactTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
        )
        .unwrap();
    assert!(matches!(
        CheckedNominalInheritanceGraphV1::validate(
            fixture.records.values(),
            &fixture,
            &mut meter()
        ),
        Err(InheritanceGraphError::ExactIdentity(_))
    ));
    fixture.exacts.insert(value.exact, old);
    fixture.origins.remove(&value.source);
    assert!(matches!(
        CheckedNominalInheritanceGraphV1::validate(
            fixture.records.values(),
            &fixture,
            &mut meter()
        ),
        Err(InheritanceGraphError::Foundation(
            "missing foundation origin"
        ))
    ));
}

#[test]
fn graph_and_query_share_resource_limits_and_reject_duplicate_nodes() {
    let mut fixture = Fixture::default();
    let base = fixture.add("Base", SourceNominalKind::Class, &[]);
    let child = fixture.add("Child", SourceNominalKind::Class, &[]);
    fixture.edges(child, Some(base), &[]);
    for limits in [
        DecodeLimits {
            decoded_edges: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_recursion: 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            CheckedNominalInheritanceGraphV1::validate(
                fixture.records.values(),
                &fixture,
                &mut BudgetMeter::new(limits)
            ),
            Err(InheritanceGraphError::Resource(_))
        ));
    }
    let record = &fixture.records[&base.exact];
    assert!(matches!(
        CheckedNominalInheritanceGraphV1::validate([record, record], &fixture, &mut meter()),
        Err(InheritanceGraphError::DuplicateNode(_))
    ));
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.records.values(),
        &fixture,
        &mut meter(),
    )
    .unwrap();
    assert!(matches!(
        graph.is_subclass(
            child.exact,
            base.exact,
            &mut BudgetMeter::new(DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            })
        ),
        Err(InheritanceQueryError::Resource(_))
    ));
}

#[test]
fn inheritance_edge_wire_is_closed_and_reader_never_repairs_duplicate_interfaces() {
    for (modality, tag) in [
        (NominalInheritanceModalityV1::Final, 1),
        (NominalInheritanceModalityV1::Open, 2),
        (NominalInheritanceModalityV1::Abstract, 3),
        (NominalInheritanceModalityV1::Interface, 4),
    ] {
        assert_eq!(encode(&modality).unwrap(), [0xa1, 0, tag]);
        assert_eq!(
            decode_canonical::<NominalInheritanceModalityV1>(
                &[0xa1, 0, tag],
                DecodeLimits::default()
            )
            .unwrap(),
            modality
        );
    }
    assert!(
        decode_canonical::<NominalInheritanceModalityV1>(&[0xa1, 0, 5], DecodeLimits::default())
            .is_err()
    );
    assert_eq!(
        encode(&DirectClassBaseV1::NoClassBase).unwrap(),
        [0xa1, 0, 1]
    );
    let mut fixture = Fixture::default();
    let base = fixture.add("Base", SourceNominalKind::Class, &[]);
    let interface = fixture.add("I", SourceNominalKind::Interface, &[]);
    let record = NominalInheritanceEdgesV1::try_new(
        base.exact,
        NominalInheritanceModalityV1::Open,
        DirectClassBaseV1::ClassBase { exact: base.exact },
        vec![interface.exact],
    )
    .unwrap();
    let bytes = encode(&record).unwrap();
    let decoded: DecodedNominalInheritanceEdgesV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(decoded.resolve(&mut fixture).unwrap(), record);
    let bad = encode(&BadInterfaces(base.exact, interface.exact)).unwrap();
    let decoded: DecodedNominalInheritanceEdgesV1 =
        decode_canonical(&bad, DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture),
        Err(InheritanceEdgeResolutionError::Order(_))
    ));
}

struct BadInterfaces(
    scoop_identity::PersistentExactTypeId,
    scoop_identity::PersistentExactTypeId,
);
impl WireEncode for BadInterfaces {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.0.encode(encoder)?;
        encoder.field(2)?;
        NominalInheritanceModalityV1::Open.encode(encoder)?;
        encoder.field(3)?;
        DirectClassBaseV1::NoClassBase.encode(encoder)?;
        encoder.field(4)?;
        wire::sequence(encoder, &[self.1, self.1])
    }
}
