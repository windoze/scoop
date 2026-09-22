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
        assert_eq!(
            protocols.fundamental_types().unit().provider(),
            CoreBuiltinNominal::Unit.declaration_key().origin()
        );
    }
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
    pending.register_authority(ConeIdentity::CORE).unwrap();
    if origin != ConeIdentity::CORE {
        pending.register_authority(origin).unwrap();
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
