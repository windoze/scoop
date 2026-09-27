use super::*;
use scoop_identity::{ConeIdentity, PendingIdentityValidation};
use scoop_wire::{WirePath, decode_canonical, encode};

mod origins;

#[test]
fn actual_generic_calls_publish_applications_and_definition_locations() {
    with_consumer(
        CONSUMER,
        |output, world, provider, provider_interface, core| {
            let mut foundation =
                hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
            let mut authority = hir::CrossConeHirProductionAuthority::new(
                &foundation,
                &output.output().export.public_export_bindings,
                world,
            );
            let interface = hir::CrossConeHirInterfaceSectionV1::from_dependency_hir(
                &output,
                &[],
                &mut authority,
            )
            .unwrap();
            foundation
                .complete_cross_cone_interface_source_points(
                    output.output().export.module(),
                    &interface,
                )
                .unwrap();
            let provider_id = ConeCoordinate::new("test", "generic-provider", "1.0.0")
                .unwrap()
                .identity()
                .unwrap();
            let current = output.output().export.cone;
            let decoded =
                [core.source_foundation.as_canonical(), provider, &foundation].map(|source| {
                    decode_canonical::<hir::DecodedHirFoundation>(&encode(source).unwrap()).unwrap()
                });
            let cones = [ConeIdentity::CORE, provider_id, current];
            let mut graphs = Vec::new();
            for (index, source) in decoded.iter().enumerate() {
                let mut pending = PendingIdentityValidation::new();
                for &cone in &cones[..=index] {
                    pending.register_authority(cone).unwrap();
                }
                source.register_identities(&mut pending).unwrap();
                for dependency in &graphs {
                    pending
                        .register_external_graph_authorities(dependency)
                        .unwrap();
                }
                source.resolve_identities(&mut pending).unwrap();
                graphs.push(pending.finish().unwrap());
            }
            let mut identities = graphs.pop().unwrap();
            origins::validate(&output, &foundation, provider, core, &mut identities);
            let provider_types = hir::OdrFreeHirFoundation::try_new(provider.clone()).unwrap();
            let calls = output.committed_dependency_call_occurrences().unwrap();
            let mut observed = 0;
            let mut provider_locations = 0;
            for reference in interface.external_references().records() {
                for site in reference.call_sites().records() {
                    let decoded = decode_canonical::<hir::DecodedHirDependencyCallSiteV1>(
                        &encode(site).unwrap(),
                    )
                    .unwrap();
                    assert_eq!(
                        decoded.resolve(&mut identities, &WirePath::root()).unwrap(),
                        *site
                    );
                    let metadata = hir::SharedTypeMetadataV1 {
                        provider: provider_id,
                        identities: &identities,
                        foundation: &provider_types,
                        public: provider_interface,
                    };
                    site.validate_source_signature(reference.target(), metadata, &identities)
                        .unwrap();
                    let actual = calls
                        .iter()
                        .find(|call| call.position() == site.position())
                        .unwrap();
                    assert_eq!(
                        reference.target(),
                        hir::ExternalHirTargetV1::Callable(actual.declaration())
                    );
                    assert_eq!(site.instantiation(), actual.instantiation());
                    assert!(matches!(
                        site.instantiation(),
                        hir::HirDependencyCallInstantiationV1::Application(_)
                    ));
                    let evaluation_provider = site.origin().evaluation().source().cone();
                    let source = if evaluation_provider == current {
                        &foundation
                    } else {
                        assert_eq!(evaluation_provider, provider_id);
                        provider_locations += 1;
                        provider
                    };
                    source
                        .validate_executable_evaluation_origin(
                            evaluation_provider,
                            site.position().root,
                            site.origin().evaluation(),
                        )
                        .unwrap();

                    let direct = hir::HirDependencyCallSiteV1::try_new_with_reason(
                        site.position(),
                        site.origin().clone(),
                        site.arguments().to_vec(),
                        site.result(),
                        site.reason().clone(),
                        site.receiver(),
                    )
                    .unwrap();
                    assert!(matches!(
                        direct.validate_source_signature(reference.target(), metadata, &identities),
                        Err(hir::HirDependencyCallSignatureError::GenericDeclaration(_))
                    ));
                    let other = calls
                        .iter()
                        .find(|call| call.declaration() != actual.declaration())
                        .unwrap();
                    let wrong = hir::HirDependencyCallSiteV1::try_new_with_instantiation(
                        site.position(),
                        site.origin().clone(),
                        site.arguments().to_vec(),
                        site.result(),
                        site.reason().clone(),
                        site.receiver(),
                        other.instantiation(),
                    )
                    .unwrap();
                    assert!(matches!(
                        wrong.validate_source_signature(reference.target(), metadata, &identities),
                        Err(hir::HirDependencyCallSignatureError::ApplicationOrigin { .. })
                    ));
                    observed += 1;
                }
            }
            assert_eq!(observed, calls.len());
            assert!(
                provider_locations >= 4,
                "nested template calls retain provider locations"
            );
        },
    )
    .unwrap();
}
