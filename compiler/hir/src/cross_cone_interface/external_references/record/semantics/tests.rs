use scoop_identity::{ConeCoordinate, ConeIdentity};

use super::*;
use crate::cross_cone_interface::external_references::test_support::{
    Fixture, TargetOriginAuthority, TargetOriginAuthorityError,
};

#[test]
fn external_target_origin_must_match_its_canonical_key() {
    let fixture = Fixture::new();
    let record = fixture.signature_reference(fixture.first_alias);
    let mut authority = TargetOriginAuthority::new(ConeIdentity::CORE, fixture.provider);

    assert!(record.validate_semantics(&mut authority).is_ok());

    authority.set_target_origin(cone("other"));
    assert_eq!(
        record.validate_semantics(&mut authority),
        Err(
            ExternalHirReferenceSemanticValidationError::OriginMismatch {
                target: record.target(),
                expected: authority.target_origin(),
                actual: fixture.provider,
            }
        )
    );
}

#[test]
fn current_cone_target_is_not_an_external_reference() {
    let fixture = Fixture::new();
    let record = fixture.signature_reference(fixture.first_alias);
    let mut authority = TargetOriginAuthority::new(fixture.provider, fixture.provider);

    assert_eq!(
        record.validate_semantics(&mut authority),
        Err(
            ExternalHirReferenceSemanticValidationError::CurrentConeTarget {
                target: record.target(),
                current: fixture.provider,
            }
        )
    );
    assert_eq!(authority.origin_queries(), 0);
}

#[test]
fn target_origin_authority_failure_is_preserved() {
    let fixture = Fixture::new();
    let record = fixture.signature_reference(fixture.first_alias);
    let mut authority = TargetOriginAuthority::new(ConeIdentity::CORE, fixture.provider);
    authority.fail_on(record.target());

    assert_eq!(
        record.validate_semantics(&mut authority),
        Err(ExternalHirReferenceSemanticValidationError::TargetOrigin {
            target: record.target(),
            error: TargetOriginAuthorityError,
        })
    );
}

#[test]
fn witness_root_and_route_failures_keep_their_record_context() {
    let fixture = Fixture::new();
    let record = fixture.alias_reference(fixture.first_alias, fixture.first_route.clone());
    let mut authority = TargetOriginAuthority::new(ConeIdentity::CORE, fixture.provider);

    assert_eq!(
        record.validate_semantics(&mut authority),
        Err(ExternalHirReferenceSemanticValidationError::BindingRoot {
            target: record.target(),
            error: TargetOriginAuthorityError,
        })
    );

    authority.set_binding_root(fixture.first_root);
    assert_eq!(
        record.validate_semantics(&mut authority),
        Err(ExternalHirReferenceSemanticValidationError::Witness {
            index: 0,
            error: DependencyBindingWitnessSemanticValidationError::RouteExceedsClosure {
                hops: fixture.first_route.hops().len(),
                closure_nodes: 0,
            },
        })
    );
}

fn cone(name: &str) -> ConeIdentity {
    ConeCoordinate::new("example", name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}
