use scoop_identity::{ConeIdentity, DefinitionOriginSubject};

use super::*;

#[test]
fn protocol_surface_replays_core_and_ordinary_provider_contracts_identically() {
    for origin in [
        ConeIdentity::CORE,
        crate::core_protocol_test_support::ordinary_origin(),
    ] {
        let (surface, mut foundation) = test_support::standalone_at(origin);
        assert_eq!(
            decode(&surface).validate_against(&foundation),
            Ok(surface.clone())
        );
        let bytes = encode(&surface).unwrap();
        let validated = decode(&surface).validate_against(&foundation).unwrap();
        assert_eq!(encode(&validated).unwrap(), bytes);
        let integer = concrete_entry(surface.fundamental_types.entries(), 1);
        foundation.set_definition_origins(vec![]).unwrap();
        assert_eq!(
            decode(&surface).validate_against(&foundation),
            Err(
                CoreCompilerProtocolSurfaceValidationError::MissingDefinitionOrigin(
                    DefinitionOriginSubject::Type(integer)
                )
            )
        );
    }
}

#[test]
fn protocol_surface_preserves_role_signatures_and_rejects_substitute_providers() {
    for (origin, other) in [
        (
            ConeIdentity::CORE,
            crate::core_protocol_test_support::ordinary_origin(),
        ),
        (
            crate::core_protocol_test_support::ordinary_origin(),
            ConeIdentity::CORE,
        ),
    ] {
        let (mut surface, foundation) = test_support::standalone_at(origin);
        let original = callable_entry_ref(surface.source_location_protocol.entries(), 1).clone();
        surface.source_location_protocol.0.entries[1] =
            CoreProtocolEntryV1::Callable(CoreProtocolCallableV1::for_test(
                original.definition(),
                SignatureCallableShape::new(
                    Effect::Ordinary,
                    None,
                    vec![],
                    SignatureTypeKey::Nominal(
                        scoop_identity::CoreBuiltinNominal::Unit
                            .identity_record()
                            .id(),
                    ),
                ),
            ));
        assert_eq!(
            decode(&surface).validate_against(&foundation),
            Err(CoreCompilerProtocolSurfaceValidationError::Relation(
                CoreCompilerProtocolSurfaceRelationError::OperationSignatureMismatch(
                    IntrinsicFunctionKind::CurrentSourceLocation
                )
            ))
        );
        let (substitute, _) = test_support::standalone_at(other);
        let substitute = callable_entry_ref(substitute.source_location_protocol.entries(), 1);
        surface.source_location_protocol.0.entries[1] =
            CoreProtocolEntryV1::Callable(substitute.clone());
        let crate::CoreProtocolCallableDefinitionV1::Function(id) = substitute.definition() else {
            panic!("ordinary operation")
        };
        assert_eq!(
            decode(&surface).validate_against(&foundation),
            Err(CoreCompilerProtocolSurfaceValidationError::Callable(
                crate::CoreProtocolCallableValidationError::UnknownFunction(*id.as_array())
            ))
        );
    }
}
