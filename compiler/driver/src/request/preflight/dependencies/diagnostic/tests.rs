use super::*;
use scoop_lir::{
    ConeProductionSectionValidationError, RegistrationProductionTableV1,
    StrongRegistrationProductionValidationError,
};

#[test]
fn reader_slots_follow_dependency_order_and_semantics_keep_the_provider() {
    let order = [ConeIdentity::SINGLE_FILE, ConeIdentity::CORE];
    let envelope = slib::CrossConeArtifactClosureValidationError::Layout(Box::new(
        slib::CrossConeLayoutArtifactValidationError::Envelope {
            slot: slib::CrossConeClosureArtifactSlotV1::Dependency(1),
            source: Box::new(slib::SlibReadError::Container(
                slib::ArchiveReadError::BadMagic,
            )),
        },
    ));
    assert_eq!(
        location(&envelope, &order),
        Some((ConeIdentity::CORE, "container:$".to_owned()))
    );
    let error = slib::CrossConeArtifactClosureValidationError::Layout(Box::new(
        slib::CrossConeLayoutArtifactValidationError::Semantic {
            source: Box::new(
                slib::CrossConeLayoutSemanticClosureError::LirStrongProduction(Box::new(
                    slib::CrossConeLayoutLirStrongProductionError {
                        provider: ConeIdentity::SINGLE_FILE,
                        source: Box::new(slib::SharedLirStrongProductionError::Replay(
                            ConeProductionSectionValidationError::Registrations(
                                StrongRegistrationProductionValidationError::TableLength {
                                    table: RegistrationProductionTableV1::InitializationUnit,
                                    expected: 3,
                                    actual: 2,
                                },
                            ),
                        )),
                    },
                )),
            ),
        },
    ));
    assert_eq!(
        location(&error, &order),
        Some((
            ConeIdentity::SINGLE_FILE,
            "lir/cone-production/6/registrations/InitializationUnit".to_owned()
        ))
    );
    assert_eq!(location(&error, &[]), location(&error, &order));
}

#[cfg(unix)]
#[test]
fn structured_artifact_error_keeps_raw_locator_bytes_and_member() {
    use scoop_protocol::{DiagnosticOriginV1, HostPathCarrier};
    use std::os::unix::ffi::OsStringExt;
    let path = std::path::PathBuf::from(std::ffi::OsString::from_vec(
        b"/dependencies/lib-\xff.slib".to_vec(),
    ));
    let error = Error::ArtifactClosure {
        input: super::super::super::ExplicitDependencyArtifactInput {
            role: super::super::super::ExplicitDependencyRole::Support,
            index: 2,
            path: path.clone(),
        },
        semantic_path: "lir/cone-production/5/registrations".to_owned(),
        source: Box::new(
            slib::CrossConeArtifactClosureValidationError::MissingCompletedCurrentArtifact,
        ),
    };
    let error = crate::SingleConeProductionError::Validation(
        crate::SingleConeDependencyValidationError::ExplicitDependencies(Box::new(error)),
    );
    let actual = error.structured_diagnostics().unwrap();
    assert_eq!(actual.len(), 1);
    assert_eq!(
        actual[0].origin(),
        &DiagnosticOriginV1::artifact_path(
            HostPathCarrier::from_path(&path).unwrap(),
            "lir/cone-production/5/registrations".to_owned()
        )
        .unwrap()
    );
    assert_eq!(actual[0].message(), error.to_string());
}
