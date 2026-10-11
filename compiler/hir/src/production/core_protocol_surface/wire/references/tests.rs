use scoop_identity::ConeIdentity;

use super::*;

#[test]
fn protocol_enum_members_require_an_actual_source_enum_owner() {
    for origin in [
        ConeIdentity::CORE,
        crate::core_protocol_test_support::ordinary_origin(),
    ] {
        let (surface, foundation) = test_support::standalone_at(origin);
        let option = surface.option_protocol().option;
        assert_eq!(
            require_source_enum_owner(
                &foundation,
                Some(NominalDeclarationOwner::GenericTemplate(option))
            ),
            Ok(())
        );
        let boolean = surface.fundamental_types().boolean;
        assert_eq!(
            require_source_enum_owner(
                &foundation,
                Some(NominalDeclarationOwner::Concrete(boolean))
            ),
            Err(CoreCompilerProtocolSurfaceValidationError::ExpectedSourceEnumOwner)
        );
        assert_eq!(
            require_source_enum_owner(&foundation, None),
            Err(CoreCompilerProtocolSurfaceValidationError::GeneratedEnumMember)
        );
        let empty = CanonicalHirFoundation::empty();
        assert_eq!(
            require_source_enum_owner(
                &empty,
                Some(NominalDeclarationOwner::GenericTemplate(option))
            ),
            Err(CoreCompilerProtocolSurfaceValidationError::UnknownGenericType(*option.as_array()))
        );
    }
}
