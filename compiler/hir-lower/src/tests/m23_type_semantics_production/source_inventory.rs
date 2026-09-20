use super::*;
use scoop_identity::{PendingIdentityValidation, ValidatedIdentityGraph};
use scoop_wire::{WireDecode, WireEncode, decode_canonical, encode};

fn decoded<T: WireDecode>(value: &impl WireEncode) -> T {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

fn identity_closure(output: &hir::OrdinaryHirOutput<'_>) -> ValidatedIdentityGraph {
    let core = trusted_core();
    let core_foundation: hir::DecodedHirFoundation = decoded(&core.foundation);
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    core_foundation.register_identities(&mut pending).unwrap();
    core_foundation.resolve_identities(&mut pending).unwrap();
    let core_graph = pending.finish().unwrap();

    let foundation = hir::CanonicalHirFoundation::from_ordinary_output(output).unwrap();
    let foundation: hir::DecodedHirFoundation = decoded(&foundation);
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_authority(output.output().export.cone)
        .unwrap();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    foundation.register_identities(&mut pending).unwrap();
    pending
        .register_external_graph_authorities(&core_graph)
        .unwrap();
    foundation.resolve_identities(&mut pending).unwrap();
    pending.finish().unwrap()
}

#[test]
fn ordinary_source_inventories_resolve_against_real_foundation_bytes() {
    let output = lower_public_nominals();
    let public = public_interface(&output);
    let production = produce_cross_cone_type_semantics(&output, &public).unwrap();
    let source = production.foundation();
    let mut identities = identity_closure(&output);
    let mut meter = BudgetMeter::new(DecodeLimits::default());

    let roots =
        hir::CanonicalSourceNominalIdsV1::try_new(source.source_roots().to_vec(), &mut meter)
            .unwrap();
    let roots = decoded::<hir::DecodedCanonicalSourceNominalIdsV1>(&roots)
        .resolve(&mut identities, &mut meter)
        .unwrap();
    assert_eq!(roots.values(), source.source_roots());

    let nominals = hir::CanonicalTypeSourceNominalsV1::try_new(
        source
            .source_nominals()
            .map(|(owner, access)| hir::TypeSourceNominalV1::new(owner, access.clone()))
            .collect(),
        &mut meter,
    )
    .unwrap();
    let restored = decoded::<hir::DecodedCanonicalTypeSourceNominalsV1>(&nominals)
        .resolve(&mut identities, &mut meter)
        .unwrap();
    assert_eq!(restored, nominals);
    assert_eq!(restored.records().len(), roots.values().len());

    let dependencies = hir::CanonicalTypeSectionDependencyFactsV1::try_new(
        source.dependency_facts().to_vec(),
        &mut meter,
    )
    .unwrap();
    let dependencies = decoded::<hir::DecodedCanonicalTypeSectionDependencyFactsV1>(&dependencies)
        .resolve(&mut identities, &mut meter)
        .unwrap();
    assert_eq!(dependencies.records(), source.dependency_facts());

    let edges = hir::CanonicalNominalInheritanceEdgesV1::try_new(
        source.local_inheritance_edges().to_vec(),
        &mut meter,
    )
    .unwrap();
    let edges = decoded::<hir::DecodedCanonicalNominalInheritanceEdgesV1>(&edges)
        .resolve(&mut identities, &mut meter)
        .unwrap();
    assert_eq!(edges.records(), source.local_inheritance_edges());
    let graph = hir::CheckedNominalInheritanceGraphV1::validate_with_source_roots(
        edges.records().iter(),
        roots.values().iter().copied(),
        source,
        &mut meter,
    )
    .unwrap();
    for record in production.section().inheritance().records() {
        graph
            .validate_nominal_domains(record.owner(), record.domains(), &mut meter)
            .unwrap();
    }
}
