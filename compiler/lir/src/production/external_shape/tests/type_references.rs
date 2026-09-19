//! Reference constituents must not be mistaken for completed selections.

use super::*;
use crate::{
    DecodedOptionalStrongTypeDescriptorRefV2, DecodedStrongTypeDescriptorRefV2,
    DecodedStrongTypeDispatchCallableRefV2, StrongExternalLirBridgeSurfaceV1,
    StrongRegistrationIdentitySurfaceV1, StrongTypeDescriptorRefV2,
    StrongTypeDispatchCallableRefV2, StrongTypeReferenceDefinitionsV2,
    StrongTypeReferenceResolutionErrorV2 as Error,
};

fn checked(subject: ExternalStrongShapeSubjectV1) -> StrongShapeDefinitionRefV1 {
    StrongShapeDefinitionRefV1::from_foundation(
        subject,
        &foundation(subject, true, 1),
        &mut meter(),
    )
    .unwrap()
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
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

#[test]
fn descriptor_and_body_references_bind_the_same_provider_definition() {
    let all = subjects();
    let definitions = [checked(all[0]), checked(all[3])];
    let catalog =
        StrongTypeReferenceDefinitionsV2::new(ConeIdentity::CORE, &definitions, &mut meter())
            .unwrap();
    let (consumer, registrations, core) = empty_consumer();
    let ExternalStrongShapeSubjectV1::TypeDescriptor(exact) = all[3] else {
        panic!("descriptor fixture");
    };
    let descriptor = StrongTypeDescriptorRefV2::DependencyExternal {
        provider: ConeIdentity::SINGLE_FILE,
        exact,
    };
    let resolved = catalog
        .resolve_descriptor(
            decoded(&descriptor),
            &consumer,
            &registrations,
            &core,
            &mut meter(),
        )
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
            .resolve_dispatch_callable(decoded(&callable), &consumer, &core, &mut meter())
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
            .resolve_optional_descriptor(
                decoded(&optional),
                &consumer,
                &registrations,
                &core,
                &mut meter()
            )
            .unwrap(),
        Some(descriptor)
    );
}

#[test]
fn physical_catalog_rejects_duplicate_local_and_wrong_role_definitions() {
    let all = subjects();
    let definition = checked(all[3]);
    assert!(matches!(
        StrongTypeReferenceDefinitionsV2::new(
            ConeIdentity::CORE,
            &[definition, definition],
            &mut meter()
        ),
        Err(Error::DuplicateDefinition(_))
    ));
    assert!(matches!(
        StrongTypeReferenceDefinitionsV2::new(
            ConeIdentity::SINGLE_FILE,
            &[definition],
            &mut meter()
        ),
        Err(Error::CurrentConeDefinition(_))
    ));
    for wrong in [all[1], all[5], all[6], all[9]] {
        assert!(
            matches!(StrongTypeReferenceDefinitionsV2::new(ConeIdentity::CORE, &[checked(wrong)], &mut meter()), Err(Error::UnexpectedSubject(subject)) if subject == wrong)
        );
    }
}

#[test]
fn an_imported_symbol_request_is_not_a_local_definition() {
    let definition = checked(subjects()[3]);
    let catalog =
        StrongTypeReferenceDefinitionsV2::new(ConeIdentity::CORE, &[definition], &mut meter())
            .unwrap();
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
            .resolve_descriptor(
                decoded(&reference),
                &consumer,
                &registrations,
                &core,
                &mut meter()
            )
            .unwrap(),
        reference
    );
}

#[test]
fn reader_rejects_provider_relabeling_and_local_or_core_fallbacks() {
    let all = subjects();
    let definitions = [checked(all[0]), checked(all[3])];
    let catalog =
        StrongTypeReferenceDefinitionsV2::new(ConeIdentity::CORE, &definitions, &mut meter())
            .unwrap();
    let (consumer, registrations, core) = empty_consumer();
    let ExternalStrongShapeSubjectV1::TypeDescriptor(exact) = all[3] else {
        panic!("descriptor fixture");
    };
    let wrong = StrongTypeDescriptorRefV2::DependencyExternal {
        provider: ConeIdentity::CORE,
        exact,
    };
    assert!(matches!(
        catalog.resolve_descriptor(
            decoded(&wrong),
            &consumer,
            &registrations,
            &core,
            &mut meter()
        ),
        Err(Error::UnknownDependencyDescriptor { .. })
    ));
    assert!(matches!(
        catalog.resolve_descriptor(
            decoded(&StrongTypeDescriptorRefV2::Local(exact)),
            &consumer,
            &registrations,
            &core,
            &mut meter()
        ),
        Err(Error::UnknownLocalDescriptor(_))
    ));
    assert!(matches!(
        catalog.resolve_descriptor(
            decoded(&StrongTypeDescriptorRefV2::CoreExternal(exact)),
            &consumer,
            &registrations,
            &core,
            &mut meter()
        ),
        Err(Error::UnknownCoreDescriptor(_))
    ));
    let PersistentSymbolKey::CallableBody(body) = definitions[0].symbol().key() else {
        panic!("body fixture");
    };
    let wrong = StrongTypeDispatchCallableRefV2::DependencyExternal {
        provider: ConeIdentity::CORE,
        body,
    };
    assert!(matches!(
        catalog.resolve_dispatch_callable(decoded(&wrong), &consumer, &core, &mut meter()),
        Err(Error::UnknownDependencyCallable { .. })
    ));
    assert!(matches!(
        catalog.resolve_dispatch_callable(
            decoded(&StrongTypeDispatchCallableRefV2::Local(body)),
            &consumer,
            &core,
            &mut meter()
        ),
        Err(Error::UnknownLocalCallable(_))
    ));
}

#[test]
fn old_core_descriptor_cannot_be_reencoded_as_general_dependency() {
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
    let catalog =
        StrongTypeReferenceDefinitionsV2::new(producer, &[definition], &mut meter()).unwrap();
    let (consumer, registrations, _) = consumer_at(producer);
    let core = StrongExternalLirBridgeSurfaceV1::try_new(
        producer,
        vec![crate::StrongExternalLirBridgeV1::TypeDescriptor(
            crate::StrongExternalTypeDescriptorBridgeV1::new(exact).unwrap(),
        )],
    )
    .unwrap();
    let old = StrongTypeDescriptorRefV2::CoreExternal(exact);
    assert_eq!(
        catalog
            .resolve_descriptor(
                decoded(&old),
                &consumer,
                &registrations,
                &core,
                &mut meter()
            )
            .unwrap(),
        old
    );
    let changed = StrongTypeDescriptorRefV2::DependencyExternal {
        provider: ConeIdentity::SINGLE_FILE,
        exact,
    };
    assert!(
        matches!(catalog.resolve_descriptor(decoded(&changed), &consumer, &registrations, &core, &mut meter()), Err(Error::CoreDescriptorPartition(actual)) if actual == exact)
    );
}

#[test]
fn absent_runtime_and_catalog_queries_keep_the_consumer_and_budget() {
    let definition = checked(subjects()[3]);
    let no_heap = DecodeLimits {
        logical_heap_bytes: 0,
        ..DecodeLimits::default()
    };
    assert!(matches!(
        StrongTypeReferenceDefinitionsV2::new(
            ConeIdentity::CORE,
            &[definition],
            &mut BudgetMeter::new(no_heap)
        ),
        Err(Error::Resource(_))
    ));
    let catalog =
        StrongTypeReferenceDefinitionsV2::new(ConeIdentity::CORE, &[definition], &mut meter())
            .unwrap();
    let (consumer, registrations, core) = empty_consumer();
    assert_eq!(
        catalog
            .resolve_optional_descriptor(
                DecodedOptionalStrongTypeDescriptorRefV2::Absent,
                &consumer,
                &registrations,
                &core,
                &mut meter()
            )
            .unwrap(),
        None
    );
    let other = crate::OdrFreeLirFoundation::try_new(
        ConeIdentity::SINGLE_FILE,
        crate::CanonicalLirFoundation::empty(),
    )
    .unwrap();
    assert!(matches!(
        catalog.resolve_optional_descriptor(
            DecodedOptionalStrongTypeDescriptorRefV2::Absent,
            &other,
            &registrations,
            &core,
            &mut meter()
        ),
        Err(Error::ProducerMismatch { .. })
    ));
    let runtime = crate::RuntimeFunction::NoGc(crate::NoGcRuntimeFunction::Trap);
    assert_eq!(
        catalog
            .resolve_dispatch_callable(
                DecodedStrongTypeDispatchCallableRefV2::Runtime(runtime),
                &consumer,
                &core,
                &mut meter()
            )
            .unwrap(),
        StrongTypeDispatchCallableRefV2::Runtime(runtime)
    );
    let ExternalStrongShapeSubjectV1::TypeDescriptor(exact) = definition.subject() else {
        panic!("descriptor fixture");
    };
    let reference: DecodedStrongTypeDescriptorRefV2 =
        decoded(&StrongTypeDescriptorRefV2::DependencyExternal {
            provider: ConeIdentity::SINGLE_FILE,
            exact,
        });
    let no_work = DecodeLimits {
        validation_work_units: 0,
        ..DecodeLimits::default()
    };
    assert!(matches!(
        catalog.resolve_descriptor(
            reference,
            &consumer,
            &registrations,
            &core,
            &mut BudgetMeter::new(no_work)
        ),
        Err(Error::Resource(_))
    ));
}
