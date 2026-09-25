use super::*;
use std::fmt::Write;

#[test]
fn source_reference_closure_golden_preserves_occurrence_positions() {
    with_template(|template| {
        let bound = template
            .bind_reference_occurrences(&WirePath::root())
            .unwrap();
        let mut actual = String::new();
        for occurrence in bound.occurrences() {
            let body = occurrence.body();
            let span = body.definition_origin.origin().span();
            writeln!(
                actual,
                "{:?}[{}] {:?} {} @ {}..{}",
                occurrence.source().kind(),
                occurrence.index(),
                body.site,
                attachment(body.attachment),
                span.start_byte(),
                span.end_byte(),
            )
            .unwrap();
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-type-source-defaults/reference-closure.snap");
        if std::env::var_os("SCOOP_UPDATE_DEFAULT_REFERENCE_SNAPSHOTS").is_some() {
            std::fs::write(&path, &actual).unwrap();
        }
        assert_eq!(actual, std::fs::read_to_string(path).unwrap());
    });
}

fn attachment(value: hir::DefaultBodyReferenceAttachmentV1<'_>) -> String {
    use hir::{DefaultBodyReferenceAttachmentV1 as A, DefaultBodyReferenceMetadataV1 as M};
    match value {
        A::Expression { index, .. } => format!("expression[{index}]"),
        A::Metadata(metadata) => match metadata {
            M::TemplateLocal { index, .. } => format!("local[{index}]"),
            M::Body(_) => "body".into(),
            M::Statement(_) => "statement".into(),
            M::Pattern(_) => "pattern".into(),
            M::Assignment(_) => "assignment".into(),
            M::WhenArm(_) => "when-arm".into(),
            M::Catch(_) => "catch".into(),
            M::Iterator(_) => "iterator".into(),
            M::BindingPlan(_) => "binding-plan".into(),
            M::BindingAction(_) => "binding-action".into(),
            M::Capture(_) => "capture".into(),
            M::LocalFunction(_) => "local-function".into(),
        },
    }
}
