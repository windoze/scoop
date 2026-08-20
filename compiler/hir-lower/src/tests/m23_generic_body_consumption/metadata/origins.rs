use super::*;
use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
    ValidatedIdentityGraph,
};

pub(super) fn validate(
    output: &hir::DependencyHirOutput,
    foundation: &hir::CanonicalHirFoundation,
    provider: &hir::CanonicalHirFoundation,
    core: &crate::tests::m23_ordinary_core_only::support::TrustedCoreFixture,
    identities: &mut ValidatedIdentityGraph,
) {
    let coordinate = ConeCoordinate::new("test", "scoop-hir-lower", "0.0.0").unwrap();
    let dependencies = [core.source_foundation.as_ref(), provider];
    let bytes = encode(foundation).unwrap();
    let decoded: hir::DecodedHirFoundation = decode_canonical(&bytes).unwrap();
    let restored = decoded
        .validate_with_dependency_sources(&coordinate, identities, &dependencies)
        .unwrap();
    assert_eq!(encode(&restored).unwrap(), bytes);
    let decoded: hir::DecodedHirFoundation = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.validate_with_dependency_sources(&coordinate, identities, &[]),
        Err(hir::HirFoundationValidationError::Origin(
            hir::DefinitionOriginValidationError::MissingSourceAnchor { .. }
        ))
    ));

    let local = output.output().local.module();
    let root = local
        .functions
        .iter()
        .find(|(_, function)| function.name == "identity")
        .unwrap()
        .1
        .materialization;
    let other = local
        .functions
        .iter()
        .find(|(_, function)| function.name == "choose")
        .unwrap()
        .1
        .materialization;
    assert!(matches!(
        root.template(),
        CallableTemplateOwner::GenericFunction(_)
    ));
    let validate = |root, part, dependencies: &[&hir::CanonicalHirFoundation]| {
        foundation.validate_declaration_type_position(
            local.cone,
            hir::HirDependencyTypePositionV1::CallableSignature(root, part),
            dependencies,
        )
    };
    use hir::DeclarationTypeSiteValidationError as Error;
    use hir::HirCallableTypePositionV1 as Part;
    for part in [Part::Parameter(0), Part::Result] {
        validate(root, part, &dependencies).unwrap();
    }
    assert!(matches!(
        validate(root, Part::Parameter(0), &[]),
        Err(Error::MissingIdentity { .. })
    ));
    for part in [Part::Receiver, Part::Parameter(99)] {
        assert!(matches!(
            validate(root, part, &dependencies),
            Err(Error::SignaturePosition(..))
        ));
    }
    for context in [
        CallableMaterializationContext::NoSubstitution,
        other.context(),
    ] {
        let wrong = CallableMaterialization::new(root.template(), context);
        assert!(matches!(
            validate(wrong, Part::Parameter(0), &dependencies),
            Err(Error::Materialization(actual)) if actual == wrong
        ));
    }
}
