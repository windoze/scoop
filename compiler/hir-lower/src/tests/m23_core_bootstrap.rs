use std::path::PathBuf;

use scoop_ast::{
    AllParsedSources, CurrentConeParsedSources, CurrentSourceDiagnosticContext, CurrentSourceText,
    IdentifiedParsedSource, NonEmptyVec,
};
use scoop_identity::{ConeIdentity, NormalizedSourcePath, SourceIdentity};

use super::complete_core_file;
use crate::{CoreBootstrapSourceError, CoreBootstrapSources, lower_core_bootstrap};

#[test]
fn core_bootstrap_lowers_directly_from_the_atomic_parser_product() {
    let parsed = parsed_source(ConeIdentity::CORE);
    let input = CoreBootstrapSources::try_new(&parsed).unwrap();

    let output = lower_core_bootstrap(&input).unwrap();

    assert_eq!(output.export.cone, ConeIdentity::CORE);
    assert_eq!(output.export.source_files.len(), 1);
    let source = &output.export.source_files[0];
    assert_eq!(source.identity.cone(), ConeIdentity::CORE);
    assert_eq!(source.identity.logical_path().as_str(), "src/core.scoop");
    assert_eq!(source.name, "<core>");
    assert!(source.source.is_empty());
    let requirements = scoop_hir::PublicNominalShapeRequirementsV1::from_public_bindings(
        output.export.cone,
        &output.export.public_export_bindings,
        &output.export.export_binding_identities,
    )
    .unwrap();
    let plan = output.local.materialization();
    assert_eq!(plan.roots().len(), requirements.roots().len());
    assert!(
        plan.roots()
            .iter()
            .all(|root| { output.local.exact_type_identities[root.ty()].id() == root.exact() })
    );
    assert!(
        plan.roots()
            .iter()
            .any(|root| { root.boxed_value() == scoop_hir::LocalBoxedValueRequirement::Required })
    );
    assert!(plan.roots().iter().any(|root| {
        root.boxed_value() == scoop_hir::LocalBoxedValueRequirement::NotApplicable
    }));
}

#[test]
fn core_bootstrap_input_rejects_a_non_core_parser_product() {
    let ordinary = scoop_identity::ConeCoordinate::new("test", "ordinary", "0.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let parsed = parsed_source(ordinary);

    assert!(matches!(
        CoreBootstrapSources::try_new(&parsed),
        Err(CoreBootstrapSourceError::NotCore(actual)) if actual == ordinary
    ));
}

fn parsed_source(cone: ConeIdentity) -> CurrentConeParsedSources {
    let identity =
        SourceIdentity::new(cone, NormalizedSourcePath::new("src/core.scoop").unwrap()).unwrap();
    CurrentConeParsedSources::try_new(
        AllParsedSources::try_new(NonEmptyVec::new(
            IdentifiedParsedSource::new(identity.clone(), complete_core_file()),
            Vec::new(),
        ))
        .unwrap(),
        NonEmptyVec::new(
            CurrentSourceText::new(identity.clone(), String::new()),
            Vec::new(),
        ),
        NonEmptyVec::new(
            CurrentSourceDiagnosticContext::new(identity, PathBuf::from("<core>")),
            Vec::new(),
        ),
    )
    .unwrap()
}
