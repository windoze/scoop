use scoop_hir::{
    DefinitionSourceLocationValidationError, ExportDefinitionSourceSemanticValidationError,
    ExportDefinitionSourceSetSemanticValidationError,
};

use super::*;
use crate::{
    CrossConeClosureDefinitionSourceError, CrossConeClosureIdentityError,
    CrossConeClosureSourceInterfaceError,
};

mod fixture;
mod origins;
use fixture::{Case, Fixture};

#[test]
fn ordinary_default_locations_follow_direct_transitive_and_diamond_providers() {
    for case in [Case::Direct, Case::Transitive, Case::Diamond] {
        let fixture = Fixture::new(case);
        let sources = fixture.closed().validate_definition_sources().unwrap();
        let checked = sources
            .validate_nominal_surfaces()
            .unwrap()
            .validate_property_surfaces()
            .unwrap()
            .validate_callable_surfaces()
            .unwrap()
            .validate_type_alias_surfaces()
            .unwrap()
            .validate_source_interfaces()
            .unwrap();
        let current = checked.artifact(fixture.current).unwrap();
        let template = &current.hir_interface().default_templates().records()[0];
        assert_eq!(
            template.definition_origin().origin().source().cone(),
            fixture.current
        );
        assert_eq!(
            template
                .body()
                .value()
                .definition_origin()
                .origin()
                .source()
                .cone(),
            fixture.foreign
        );
    }
}

#[test]
fn artifact_identity_rejects_retained_metadata_from_an_unreachable_source_provider() {
    let fixture = Fixture::new(Case::Unreachable);
    let Err(CrossConeClosureIdentityError::Artifact { identity, source }) =
        fixture.profiled().validate_identities()
    else {
        panic!("source copies must not create dependency reachability")
    };
    assert_eq!(identity, fixture.current);
    assert!(
        source
            .to_string()
            .contains(&format!("missing cone identity {}", fixture.foreign))
    );
}

#[test]
fn ordinary_default_locations_require_actual_provider_records_despite_retained_copies() {
    let fixture = Fixture::new(Case::RetainedCopy);
    let Err(error) = fixture.direct_front().validate_definition_sources(&[]) else {
        panic!("retained metadata must not substitute the actual provider foundation")
    };
    assert!(
        error
            .to_string()
            .contains("is not reachable from this artifact")
    );
    assert!(matches!(error,
        CrossConeHirDefinitionSourceSurfaceError::UnavailableProvider { index, provider }
        if index == fixture.source_index && provider == fixture.foreign
    ));
}

#[test]
fn ordinary_default_locations_check_the_actual_provider_source_points() {
    let fixture = Fixture::new(Case::MissingPoint);
    let Err(CrossConeClosureDefinitionSourceError::Artifact { identity, source }) =
        fixture.closed().validate_definition_sources()
    else {
        panic!("foreign inline origins require actual provider byte positions")
    };
    assert_eq!(identity, fixture.current);
    assert!(
        source
            .to_string()
            .contains("does not declare byte offset 1")
    );
    assert!(matches!(*source,
        CrossConeHirDefinitionSourceSurfaceError::DefinitionSources(
            ExportDefinitionSourceSetSemanticValidationError::Source {
                index,
                error: ExportDefinitionSourceSemanticValidationError::Foundation(
                    DefinitionSourceLocationValidationError::MissingSourcePoint { byte_offset: 1, .. }
                ),
            }
        ) if index == fixture.source_index
    ));
}

#[test]
fn shared_location_membership_does_not_authorize_a_foreign_parameter_origin() {
    let fixture = Fixture::new(Case::ForeignParameter);
    let aliases = fixture
        .closed()
        .validate_definition_sources()
        .unwrap()
        .validate_nominal_surfaces()
        .unwrap()
        .validate_property_surfaces()
        .unwrap()
        .validate_callable_surfaces()
        .unwrap()
        .validate_type_alias_surfaces()
        .unwrap();
    let Err(CrossConeClosureSourceInterfaceError::Artifact { identity, source }) =
        aliases.validate_source_interfaces()
    else {
        panic!("parameter source must still belong to its own declaration")
    };
    assert_eq!(identity, fixture.current);
    assert!(matches!(*source,
        CrossConeHirSourceInterfaceSurfaceError::SourceInterfaces(
            CallableSourceInterfaceSetSemanticValidationError::Record {
                index: 0,
                error: CallableSourceInterfaceSemanticValidationError::DefinitionOriginCone {
                    index: 0, expected, actual,
                },
            }
        ) if expected == fixture.current && actual == fixture.foreign
    ));
}
