use super::*;
use crate::cross_cone_type_bridge::tests::support::Fixture;
use scoop_identity::{CborIdentityRecord, NonEmptyVec, PendingIdentityValidation};

#[test]
fn all_finite_helper_payload_edges_collapse_to_the_exact_source() {
    let fixture = Fixture::new();
    for record in [
        fixture.boxed_export(),
        fixture.step_export(),
        fixture.slot_export(),
    ] {
        let references =
            MirTypeBridgeSemanticReferencesV1::of_type(&record, &fixture.graph).unwrap();
        assert_eq!(
            references.targets(),
            &[MirTypeBridgeTargetV1::Type(fixture.payload.id())]
        );
    }
    assert!(
        MirTypeBridgeSemanticReferencesV1::of_type(&fixture.empty_export(), &fixture.graph,)
            .unwrap()
            .targets()
            .is_empty()
    );
}

#[test]
fn a_shape_family_requires_source_and_every_finite_helper() {
    let fixture = Fixture::new();
    let types = CanonicalParamFreeMirTypeExportsV1::try_new(vec![
        fixture.empty_export(),
        fixture.boxed_export(),
        fixture.step_export(),
        fixture.slot_export(),
    ])
    .unwrap();
    let shape = ParamFreeMirShapeSupportV1::try_new(
        MirShapeSupportAuthority {
            identities: &fixture.graph,
            types: &types,
        },
        fixture.empty.id(),
        fixture.payload.id(),
        MirBoxedShapeSupportV1::Available(fixture.boxed_export().exact()),
        fixture.step_export().exact(),
        fixture.slot_export().exact(),
    )
    .unwrap();
    assert_eq!(
        MirTypeBridgeSemanticReferencesV1::of_shape(&shape, &fixture.graph,)
            .unwrap()
            .targets(),
        expected(
            types
                .records()
                .iter()
                .map(|record| { MirTypeBridgeTargetV1::Type(record.exact()) })
                .collect()
        )
    );
}

#[test]
fn nested_tuple_fields_expand_only_to_nominal_leaves() {
    let fixture = Fixture::new();
    let (graph, exacts) = structural_graph(&fixture);
    let record = ParamFreeMirTypeExportV1::try_new(
        MirTypeBridgeAuthority {
            identities: &graph,
            foundation: &fixture.foundation,
        },
        fixture.payload.id(),
        fixture.empty_export().origin().clone(),
        MirTypeFactsV1::try_new(MirValueKindV1::NonZeroValue, MirGcKindV1::GcFree).unwrap(),
        MirTypeRepresentationV1::Struct {
            fields: vec![MirRepresentationFieldV1 {
                field: fixture.fields[0].id(),
                value: exacts[1],
            }],
            c_layout: MirTypeCLayoutPolicyV1::Ordinary,
            interior_mutable: false,
        },
        fixture.empty_export().base_and_interfaces().clone(),
    )
    .unwrap();
    assert_eq!(
        MirTypeBridgeSemanticReferencesV1::of_type(&record, &graph)
            .unwrap()
            .targets(),
        &[MirTypeBridgeTargetV1::Type(structural_leaf(&fixture))]
    );
}

#[test]
fn structural_fields_preserve_types_without_standalone_materialization() {
    let fixture = Fixture::new();
    let (graph, exacts) = structural_graph(&fixture);
    for exact in &exacts[..5] {
        let mut collector = collector::Collector::new(&graph);
        assert!(matches!(collector.exact(*exact),
            Err(MirTypeBridgeReferenceError::StructuralExecutionGate(id)) if id == *exact));
    }
    for exact in &exacts[2..5] {
        let mut collector = collector::Collector::new(&graph);
        collector.field(*exact).unwrap();
        assert!(collector.finish().unwrap().targets().is_empty());
    }
}

#[test]
fn nominal_applications_require_their_own_materialized_type() {
    let fixture = Fixture::new();
    let (graph, exacts) = structural_graph(&fixture);
    let application = exacts[5];
    let mut collector = collector::Collector::new(&graph);
    collector.exact(application).unwrap();
    collector.field(application).unwrap();
    assert_eq!(
        collector.finish().unwrap().targets(),
        &[MirTypeBridgeTargetV1::Type(application)]
    );
}

fn structural_graph(fixture: &Fixture) -> (ValidatedIdentityGraph, Vec<PersistentExactTypeId>) {
    let leaf = structural_leaf(fixture);
    let inner: CborIdentityRecord<PersistentExactTypeId, _> =
        CborIdentityRecord::from_key(ExactTypeKey::Tuple(NonEmptyVec::from_first(leaf, [leaf])))
            .unwrap();
    let outer = CborIdentityRecord::from_key(ExactTypeKey::Tuple(NonEmptyVec::from_first(
        inner.id(),
        [leaf],
    )))
    .unwrap();
    let pointer = CborIdentityRecord::from_key(ExactTypeKey::RawPointer(leaf)).unwrap();
    let function = CborIdentityRecord::from_key(ExactTypeKey::Function {
        effect: scoop_identity::Effect::Ordinary,
        parameters: vec![leaf],
        result: leaf,
    })
    .unwrap();
    let native = CborIdentityRecord::from_key(ExactTypeKey::NativeFunctionPointer {
        calling_convention: scoop_identity::CallingConvention::C,
        parameters: vec![leaf],
        result: leaf,
    })
    .unwrap();
    let generic = generic_nominal();
    let application = CborIdentityRecord::from_key(ExactTypeKey::NominalApplication {
        origin: generic.id(),
        arguments: NonEmptyVec::from_first(leaf, []),
    })
    .unwrap();
    let records = vec![inner, outer, pointer, function, native, application];
    let ids = records.iter().map(CborIdentityRecord::id).collect();
    let mut hir = scoop_hir::CanonicalHirFoundation::empty();
    hir.set_generic_types(vec![generic]).unwrap();
    hir.set_exact_types(records).unwrap();
    let decoded: scoop_hir::DecodedHirFoundation =
        decode_canonical(&encode(&hir).unwrap()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_external_graph_authorities(&fixture.graph)
        .unwrap();
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    (pending.finish().unwrap(), ids)
}

fn structural_leaf(fixture: &Fixture) -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(fixture.other.id())).unwrap()
}

fn generic_nominal()
-> CborIdentityRecord<scoop_identity::PersistentGenericTypeId, scoop_identity::SourceDeclarationKey>
{
    use scoop_identity::*;
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("Generic").unwrap(),
        SourceNominalKind::Struct,
        1,
    ))
    .unwrap()
}
