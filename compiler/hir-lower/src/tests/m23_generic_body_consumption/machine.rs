use super::*;
use scoop_identity::{ConeIdentity, PendingIdentityValidation};
use scoop_mir as mir;
use scoop_wire::{decode_canonical, encode};

#[test]
fn actual_generic_calls_retain_odr_roots_in_the_shared_machine_input() {
    with_consumer(CONSUMER, |output, _, provider, _, core| {
        let current = output.output().export.cone;
        let foundation = hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
        let hir_production =
            hir::CoreBootstrapInterfaceSectionV1::from_export(&output.output().export).unwrap();
        let dependencies =
            mir::SelectedExternalMirSet::try_from_callables(current, Vec::new()).unwrap();
        let lowered = scoop_mir_lower::lower_current_cone(&output, dependencies).unwrap();
        let production = scoop_mir_lower::lower_production_section(
            current,
            &hir_production,
            lowered.foundation(),
        )
        .unwrap();
        let shapes = output
            .output()
            .local
            .materialization()
            .roots()
            .iter()
            .map(|root| root.declaration().clone())
            .collect();
        let expected = encode(lowered.foundation()).unwrap();
        let input = mir::ConeMirInput::try_new(lowered, production, shapes)
            .expect("source-selected generic applications use the shared MIR input");
        assert_eq!(encode(input.foundation()).unwrap(), expected);
        let roots = input.materialization().callable_roots();
        assert_eq!(roots.len(), input.module().top_level.len());
        let odr = roots
            .iter()
            .filter(|root| matches!(root.subject(), mir::CallableSignatureSubject::Odr(_)))
            .count();
        assert_eq!(
            odr, 11,
            "provider templates and the local relay retain their instances"
        );
        for root in roots {
            assert_eq!(
                input
                    .module()
                    .meta
                    .callable_signature_subject(root.function()),
                Some(root.subject())
            );
            assert!(
                input
                    .foundation()
                    .callable_signature(root.subject())
                    .is_some()
            );
        }
        let strong = input.production().strong_callable_bridges().bridges();
        assert_eq!(strong.len(), roots.len() - odr);
        assert!(!strong.is_empty());
        assert!(matches!(
            mir::OdrFreeMirFoundation::try_new(input.foundation().clone()),
            Err(mir::OdrFreeMirFoundationError::CallableSignatureSubject(_))
        ));

        let decoded_mir = decode_canonical::<mir::DecodedMirFoundation>(&expected).unwrap();
        let decoded = [core.source_foundation.as_ref(), provider, &foundation].map(|source| {
            decode_canonical::<hir::DecodedHirFoundation>(&encode(source).unwrap()).unwrap()
        });
        let provider_id = ConeCoordinate::new("test", "generic-provider", "1.0.0")
            .unwrap()
            .identity()
            .unwrap();
        let cones = [ConeIdentity::CORE, provider_id, current];
        let mut graphs = Vec::new();
        for (index, source) in decoded.iter().enumerate() {
            let mut pending = PendingIdentityValidation::new();
            for &cone in &cones[..=index] {
                pending.register_authority(cone).unwrap();
            }
            source.register_identities(&mut pending).unwrap();
            if index == decoded.len() - 1 {
                decoded_mir.register_identities(&mut pending).unwrap();
            }
            for dependency in &graphs {
                pending
                    .register_external_graph_authorities(dependency)
                    .unwrap();
            }
            source.resolve_identities(&mut pending).unwrap();
            if index == decoded.len() - 1 {
                decoded_mir.resolve_identities(&mut pending).unwrap();
            }
            graphs.push(pending.finish().unwrap());
        }
        let mut identities = graphs.pop().unwrap();
        let restored = decoded_mir.validate(&mut identities).unwrap();
        assert_eq!(encode(&restored).unwrap(), expected);
        let decoded_production = decode_canonical::<mir::DecodedCoreBootstrapBridgeSectionV1>(
            &encode(input.production()).unwrap(),
        )
        .unwrap();
        assert_eq!(
            &decoded_production
                .validate(current, &mut identities, &restored)
                .unwrap(),
            input.production()
        );

        let incomplete = mir::CoreBootstrapBridgeSectionV1::try_new(
            current,
            mir::EntryMirBridgeBranchV1::Library,
            mir::StrongCallableBridgeSurfaceV1::try_new(strong[..strong.len() - 1].to_vec())
                .unwrap(),
        )
        .unwrap();
        let decoded = decode_canonical::<mir::DecodedCoreBootstrapBridgeSectionV1>(
            &encode(&incomplete).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            decoded.validate(current, &mut identities, &restored),
            Err(mir::MirProductionValidationError::StrongCallableCoverage { expected, actual })
                if expected == strong.len() && actual + 1 == expected
        ));
    })
    .unwrap();
}
