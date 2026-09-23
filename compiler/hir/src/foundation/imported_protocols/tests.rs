use scoop_identity::*;
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;
use crate::core_protocol_test_support::standalone_at;
use crate::{CanonicalHirFoundation, DecodedHirFoundation, OdrFreeHirFoundation};

#[test]
fn protocol_import_resolves_actual_provider_ids_in_the_shared_session() {
    for origin in [
        ConeIdentity::CORE,
        crate::core_protocol_test_support::ordinary_origin(),
    ] {
        let (surface, foundation) = standalone_at(origin);
        let imported = import(origin, &foundation);
        let protocols = ImportedCoreProtocols::import(&imported, &surface).unwrap();
        let CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(expected)) =
            surface.fundamental_types().entries()[9]
        else {
            panic!("Boolean role")
        };
        assert_eq!(imported.origin(), origin);
        assert_eq!(
            protocols.fundamental_types().boolean().persistent(),
            expected
        );
        assert_eq!(
            protocols.fundamental_types().boolean().identity(),
            imported.identity(expected).unwrap()
        );
        assert_eq!(protocols.fundamental_types().boolean().provider(), origin);
        assert_eq!(protocols.fundamental_types().array().provider(), origin);
        for callable in [
            protocols.iteration().next(),
            protocols.exceptions().initialization_cycle_thrower(),
            protocols.exceptions().throwable_constructor(),
            protocols.ffi().ptr_cast(),
            protocols.source_location().current(),
        ] {
            assert_eq!(callable.provider(), origin);
        }
        assert_eq!(
            protocols.fundamental_types().unit().provider(),
            CoreBuiltinNominal::Unit.declaration_key().origin()
        );
    }
}

#[test]
fn protocol_callable_provider_comes_from_the_actual_definition_origin() {
    let provider = crate::core_protocol_test_support::ordinary_origin();
    let (surface, foundation) = standalone_at(provider);
    let imported = import(ConeIdentity::SINGLE_FILE, &foundation);
    let protocols = ImportedCoreProtocols::import(&imported, &surface).unwrap();
    assert_ne!(imported.origin(), provider);
    assert_eq!(
        protocols
            .exceptions()
            .initialization_cycle_thrower()
            .provider(),
        provider
    );
}

#[test]
fn protocol_callable_import_requires_its_shared_definition_origin() {
    let (surface, mut foundation) = standalone_at(ConeIdentity::CORE);
    foundation.set_definition_origins(vec![]).unwrap();
    let imported = import(ConeIdentity::CORE, &foundation);
    let CoreProtocolEntryV1::Callable(callable) = &surface.iteration_protocol().entries()[1] else {
        panic!("iteration next is a callable")
    };
    assert_eq!(
        ImportedCoreProtocols::import(&imported, &surface).unwrap_err(),
        CoreProtocolImportError::MissingDefinitionOrigin {
            subject: callable.definition().origin_subject(),
        }
    );
}

#[test]
fn protocol_import_rejects_same_named_declarations_from_another_provider() {
    let (surface, _) = standalone_at(ConeIdentity::CORE);
    let (_, other) = standalone_at(crate::core_protocol_test_support::ordinary_origin());
    let imported = import(crate::core_protocol_test_support::ordinary_origin(), &other);
    let CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(expected)) =
        surface.fundamental_types().entries()[1]
    else {
        panic!("integer role")
    };
    assert_eq!(
        ImportedCoreProtocols::import(&imported, &surface).unwrap_err(),
        CoreProtocolImportError::MissingIdentity {
            kind: CoreProtocolIdentityKind::Type,
            identity: *expected.as_array(),
        }
    );
}

fn import(origin: ConeIdentity, foundation: &CanonicalHirFoundation) -> ImportedHirFoundation {
    let decoded: DecodedHirFoundation =
        decode_canonical(&encode(foundation).unwrap(), DecodeLimits::default()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    let mut providers = vec![
        ConeIdentity::CORE,
        origin,
        crate::core_protocol_test_support::ordinary_origin(),
    ];
    providers.sort_unstable();
    providers.dedup();
    for provider in providers {
        pending.register_authority(provider).unwrap();
    }
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let graph = pending.finish().unwrap();
    let mut session = SemanticIdentitySession::new();
    let (hir, _, _) = session
        .import(
            origin,
            SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]),
            &graph,
        )
        .unwrap()
        .into_parts();
    ImportedHirFoundation::from_odr_free(
        OdrFreeHirFoundation::try_new(foundation.clone()).unwrap(),
        hir,
    )
}
