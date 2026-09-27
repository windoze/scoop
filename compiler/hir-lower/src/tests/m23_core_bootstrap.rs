use std::path::PathBuf;

use scoop_ast::{
    AllParsedSources, CurrentConeParsedSources, CurrentSourceDiagnosticContext, CurrentSourceText,
    IdentifiedParsedSource, NonEmptyVec,
};
use scoop_identity::{ConeIdentity, NormalizedSourcePath, SourceIdentity};

use super::complete_core_file;
use crate::lower_core_bootstrap;

#[test]
fn core_bootstrap_lowers_directly_from_the_atomic_parser_product() {
    let parsed = parsed_source(ConeIdentity::CORE);

    let output = lower_core_bootstrap(&parsed).unwrap();

    assert_eq!(output.export.cone, ConeIdentity::CORE);
    assert_eq!(output.export.source_files.len(), 1);
    let source = &output.export.source_files[0];
    assert_eq!(source.identity.cone(), ConeIdentity::CORE);
    assert_eq!(source.identity.logical_path().as_str(), "src/core.scoop");
    assert_eq!(source.name, "<core>");
    assert!(source.source.is_empty());
    let requirements =
        scoop_hir::PublicNominalShapeRequirementsV1::from_export_hir(&output.export).unwrap();
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
