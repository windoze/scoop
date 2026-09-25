//! Reference constituents must not be mistaken for completed selections.

use super::*;
use crate::{
    StrongExternalLirBridgeSurfaceV1, StrongRegistrationIdentitySurfaceV1,
    StrongTypeDescriptorRefV2, StrongTypeDispatchCallableRefV2, StrongTypeReferenceDefinitionsV2,
    StrongTypeReferenceResolutionErrorV2 as Error,
};

fn checked(subject: ExternalStrongShapeSubjectV1) -> StrongShapeDefinitionRefV1 {
    StrongShapeDefinitionRefV1::from_foundation(subject, &foundation(subject, true, 1)).unwrap()
}

fn empty_consumer() -> (
    crate::OdrFreeLirFoundation,
    StrongRegistrationIdentitySurfaceV1,
    StrongExternalLirBridgeSurfaceV1,
) {
    consumer_at(ConeIdentity::CORE)
}

fn consumer_at(
    producer: ConeIdentity,
) -> (
    crate::OdrFreeLirFoundation,
    StrongRegistrationIdentitySurfaceV1,
    StrongExternalLirBridgeSurfaceV1,
) {
    let foundation =
        crate::OdrFreeLirFoundation::try_new(producer, crate::CanonicalLirFoundation::empty())
            .unwrap();
    let image = crate::DigestNodeV1::new(
        scoop_identity::DigestNodeKey::runtime_image(producer),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let digests = crate::StrongDigestFinalizationPlanV1::new(vec![image], &foundation).unwrap();
    let registrations =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
    let core = StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap();
    (foundation, registrations, core)
}

fn decoded<T: scoop_wire::WireDecode>(value: &impl scoop_wire::WireEncode) -> T {
    decode_canonical(&encode(value).unwrap()).unwrap()
}

#[test]
fn descriptor_and_body_references_bind_the_same_provider_definition() {
    let all = subjects();
    let definitions = [checked(all[0]), checked(all[3])];
    let catalog = StrongTypeReferenceDefinitionsV2::new(ConeIdentity::CORE, &definitions).unwrap();
    let (consumer, registrations, core) = empty_consumer();
    let ExternalStrongShapeSubjectV1::TypeDescriptor(exact) = all[3] else {
        panic!("descriptor fixture");
    };
    let descriptor = StrongTypeDescriptorRefV2::DependencyExternal {
        provider: ConeIdentity::SINGLE_FILE,
        exact,
    };
    let resolved = catalog
        .resolve_descriptor(decoded(&descriptor), &consumer, &registrations, &core)
        .unwrap();
    assert_eq!(resolved, descriptor);
    assert_eq!(catalog.descriptor_definitions(), &definitions[1..]);
    let PersistentSymbolKey::CallableBody(body) = definitions[0].symbol().key() else {
        panic!("body fixture");
    };
    let callable = StrongTypeDispatchCallableRefV2::DependencyExternal {
        provider: ConeIdentity::SINGLE_FILE,
        body,
    };
    assert_eq!(
        catalog
            .resolve_dispatch_callable(decoded(&callable), &consumer, &core)
            .unwrap(),
        callable
    );
    assert_eq!(catalog.callable_definitions(), &definitions[..1]);
    let optional = crate::OptionalStrongTypeDescriptorRefV2::DependencyExternal {
        provider: ConeIdentity::SINGLE_FILE,
        exact,
    };
    assert_eq!(
        catalog
            .resolve_optional_descriptor(decoded(&optional), &consumer, &registrations, &core)
            .unwrap(),
        Some(descriptor)
    );
}

#[test]
fn physical_catalog_rejects_duplicate_local_and_wrong_role_definitions() {
    let all = subjects();
    let definition = checked(all[3]);
    assert!(matches!(
        StrongTypeReferenceDefinitionsV2::new(ConeIdentity::CORE, &[definition, definition]),
        Err(Error::DuplicateDefinition(_))
    ));
    assert!(matches!(
        StrongTypeReferenceDefinitionsV2::new(ConeIdentity::SINGLE_FILE, &[definition]),
        Err(Error::CurrentConeDefinition(_))
    ));
    for wrong in [all[1], all[5], all[6], all[9]] {
        assert!(
            matches!(StrongTypeReferenceDefinitionsV2::new(ConeIdentity::CORE, &[checked(wrong)]), Err(Error::UnexpectedSubject(subject)) if subject == wrong)
        );
    }
}

#[test]
fn an_imported_symbol_request_is_not_a_local_definition() {
    let definition = checked(subjects()[3]);
    let catalog = StrongTypeReferenceDefinitionsV2::new(ConeIdentity::CORE, &[definition]).unwrap();
    let (_, registrations, core) = empty_consumer();
    let mut canonical = crate::CanonicalLirFoundation::empty();
    canonical
        .set_symbol_requests(PersistentSymbolRequestTable::new(vec![definition.symbol()]).unwrap());
    let consumer = crate::OdrFreeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap();
    let ExternalStrongShapeSubjectV1::TypeDescriptor(exact) = definition.subject() else {
        panic!("descriptor fixture");
    };
    let reference = StrongTypeDescriptorRefV2::DependencyExternal {
        provider: definition.provider(),
        exact,
    };
    assert_eq!(
        catalog
            .resolve_descriptor(decoded(&reference), &consumer, &registrations, &core)
            .unwrap(),
        reference
    );
}

#[test]
fn reader_rejects_provider_relabeling_and_local_or_core_fallbacks() {
    let all = subjects();
    let definitions = [checked(all[0]), checked(all[3])];
    let catalog = StrongTypeReferenceDefinitionsV2::new(ConeIdentity::CORE, &definitions).unwrap();
    let (consumer, registrations, core) = empty_consumer();
    let ExternalStrongShapeSubjectV1::TypeDescriptor(exact) = all[3] else {
        panic!("descriptor fixture");
    };
    let wrong = StrongTypeDescriptorRefV2::DependencyExternal {
        provider: ConeIdentity::CORE,
        exact,
    };
    assert!(matches!(
        catalog.resolve_descriptor(decoded(&wrong), &consumer, &registrations, &core),
        Err(Error::UnknownDependencyDescriptor { .. })
    ));
    assert!(matches!(
        catalog.resolve_descriptor(
            decoded(&StrongTypeDescriptorRefV2::Local(exact)),
            &consumer,
            &registrations,
            &core
        ),
        Err(Error::UnknownLocalDescriptor(_))
    ));
    let PersistentSymbolKey::CallableBody(body) = definitions[0].symbol().key() else {
        panic!("body fixture");
    };
    let wrong = StrongTypeDispatchCallableRefV2::DependencyExternal {
        provider: ConeIdentity::CORE,
        body,
    };
    assert!(matches!(
        catalog.resolve_dispatch_callable(decoded(&wrong), &consumer, &core),
        Err(Error::UnknownDependencyCallable { .. })
    ));
    assert!(matches!(
        catalog.resolve_dispatch_callable(
            decoded(&StrongTypeDispatchCallableRefV2::Local(body)),
            &consumer,
            &core
        ),
        Err(Error::UnknownLocalCallable(_))
    ));
}

#[test]
fn descriptor_sources_bind_the_requested_provider_and_reject_duplicate_proofs() {
    let producer = scoop_identity::ConeCoordinate::new("test", "reference-consumer", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    ))
    .unwrap();
    let definition = checked(ExternalStrongShapeSubjectV1::TypeDescriptor(exact));
    let catalog = StrongTypeReferenceDefinitionsV2::new(producer, &[definition]).unwrap();
    let (consumer, registrations, _) = consumer_at(producer);
    for provider in [ConeIdentity::CORE, definition.provider()] {
        let external = StrongExternalLirBridgeSurfaceV1::try_new(
            producer,
            vec![crate::StrongExternalLirBridgeV1::TypeDescriptor(
                crate::ExternalTypeDescriptor::new(provider, exact).unwrap(),
            )],
        )
        .unwrap();
        let reference = StrongTypeDescriptorRefV2::DependencyExternal { provider, exact };
        let result =
            catalog.resolve_descriptor(decoded(&reference), &consumer, &registrations, &external);
        if provider == definition.provider() {
            assert!(
                matches!(result, Err(Error::ConflictingDescriptorSources(actual)) if actual == exact)
            );
        } else {
            assert_eq!(result.unwrap(), reference);
            let layout_reference = StrongTypeDescriptorRefV2::DependencyExternal {
                provider: definition.provider(),
                exact,
            };
            assert_eq!(
                catalog
                    .resolve_descriptor(
                        decoded(&layout_reference),
                        &consumer,
                        &registrations,
                        &external,
                    )
                    .unwrap(),
                layout_reference
            );
        }
    }
}
