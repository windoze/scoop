use scoop_hir::IntrinsicFunctionKind;

use super::*;
use crate::CrossConeIntrinsicDeclarationError;

#[test]
fn compile_callable_gate_rejects_missing_intrinsic_roles_and_duplicate_kinds() {
    for count in [1, 2] {
        let bytes = intrinsic_artifact(count);
        let front = validate_until_property(&bytes);
        let Err(CrossConeHirCallableSurfaceError::Intrinsics(error)) =
            front.validate_callable_surface(vec![])
        else {
            panic!("shared callable gate must validate intrinsic declarations")
        };
        match count {
            1 => assert!(matches!(
                *error,
                CrossConeIntrinsicDeclarationError::MissingTypeRoles(provider) if provider == cone().identity()
            )),
            2 => assert!(matches!(
                *error,
                CrossConeIntrinsicDeclarationError::DuplicateKind(IntrinsicFunctionKind::GcCollect)
            )),
            _ => unreachable!(),
        }
    }
}

#[test]
fn intrinsic_provider_lookup_rejects_duplicate_provider_entries_and_missing_actual_roles() {
    let bytes = intrinsic_artifact(1);
    let front = validate_until_property(&bytes);
    let mut decoded = open_graph(&bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let identities = decoded
        .validate_foundation_identities(std::iter::empty())
        .unwrap();
    let provider = front.identity();
    let section = front.hir_core_production();
    assert!(matches!(
        crate::cross_cone_hir_authority::validate_intrinsic_declarations(
            front.hir_interface(),
            &identities,
            [(provider, section), (provider, section)],

        ),
        Err(CrossConeIntrinsicDeclarationError::DuplicateProvider(actual)) if actual == provider
    ));
    assert!(matches!(
        crate::cross_cone_hir_authority::validate_intrinsic_declarations(
            front.hir_interface(),
            &identities,
            [(ConeIdentity::CORE, section)],

        ),
        Err(CrossConeIntrinsicDeclarationError::MissingTypeRoles(actual)) if actual == provider
    ));
}

fn intrinsic_artifact(count: usize) -> Vec<u8> {
    let (foundation, interface, ..) = nominal_surface(cone().identity(), true, true, true);
    let bytes = cross_cone_artifact_for_with_hir_foundation(cone(), vec![], &foundation, interface);
    let front = validate_until_property(&bytes);
    let interface = front.hir_interface();
    let mut changed = 0;
    let callables = interface
        .callable_interfaces()
        .records()
        .iter()
        .map(|record| {
            if changed == count
                || !matches!(
                    record.declaration(),
                    CallableTemplateOrigin::Function(_)
                        | CallableTemplateOrigin::GenericFunction(_)
                )
            {
                return record.clone();
            }
            changed += 1;
            CallableInterfaceRecordV1::try_new(
                record.declaration(),
                record.owner(),
                record.type_parameters().clone(),
                record.receiver().cloned(),
                record.parameters().clone(),
                record.result().clone(),
                CallableSourceEffectsV1::try_new(
                    Effect::Ordinary,
                    CallableSafetyV1::Safe,
                    GcEffect::Managed,
                    CallableImplementationV1::Intrinsic(IntrinsicFunctionKind::GcCollect),
                    CallableOperatorRoleV1::None,
                    CallableInfixV1::Ordinary,
                )
                .unwrap(),
                record.modality(),
                record.access(),
                scoop_hir::CanonicalPersistentIdsV1::empty(),
                Vec::new(),
            )
            .unwrap()
        })
        .collect();
    assert_eq!(changed, count);
    let mut interface = CrossConeHirInterfaceSectionV1::new(
        interface.public_bindings().clone(),
        interface.nominal_interfaces().clone(),
        CanonicalCallableInterfacesV1::with_support(
            callables,
            interface.callable_interfaces().support_records().to_vec(),
        )
        .unwrap(),
        interface.property_interfaces().clone(),
        interface.type_aliases().clone(),
        interface.source_interfaces().clone(),
        interface.default_templates().clone(),
        interface.constants().clone(),
        interface.definition_sources().clone(),
        interface.external_references().clone(),
        interface.generic_callable_bodies().clone(),
        interface.generic_initializations().clone(),
        interface.generic_delegates().clone(),
    );
    cross_cone_artifact_for_with_hir_foundation(
        cone(),
        vec![],
        &foundation,
        encode(&interface.index_for_wire().unwrap()).unwrap(),
    )
}

fn validate_until_property(bytes: &[u8]) -> PropertyValidatedCrossConeHirFrontSections<'_> {
    let mut decoded = open_graph(bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let identities = decoded
        .validate_foundation_identities(std::iter::empty())
        .unwrap();
    decoded
        .validate_foundation_structure(identities)
        .unwrap()
        .resolve_hir_interface()
        .unwrap()
        .validate_hir_production()
        .unwrap()
        .validate_internal_hir_closures()
        .unwrap()
        .validate_definition_sources(&[])
        .unwrap()
        .validate_nominal_surface(vec![])
        .unwrap()
        .validate_property_surface(vec![])
        .unwrap()
}
