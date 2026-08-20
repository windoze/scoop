use super::*;
use crate::{DiagnosticNoteV1, DiagnosticSeverityV1};

#[test]
fn artifact_mapping_keeps_note_locations_messages_and_unrelated_origins() {
    let path = |value: &str| HostPathCarrier::from_path(std::path::Path::new(value)).unwrap();
    let origin = |value, member: &str| {
        DiagnosticOriginV1::artifact_path(path(value), member.to_owned()).unwrap()
    };
    let private = path("/staging/consumer.slib");
    let retained = path("/inputs/consumer.slib");
    let mut diagnostic = StructuredDiagnosticV1::new(
        DiagnosticSeverityV1::Error,
        "SCOOPC_BUILD_FAILED".to_owned(),
        "invalid registration".to_owned(),
        origin("/staging/consumer.slib", "lir/initialization/role"),
        vec![
            DiagnosticNoteV1::new(
                "same artifact".to_owned(),
                origin("/staging/consumer.slib", "mir/callable"),
            )
            .unwrap(),
            DiagnosticNoteV1::new(
                "another artifact".to_owned(),
                origin("/inputs/provider.slib", "hir/declaration"),
            )
            .unwrap(),
            DiagnosticNoteV1::new("tool note".to_owned(), DiagnosticOriginV1::None).unwrap(),
        ],
    )
    .unwrap();
    let unchanged_notes = diagnostic.notes()[1..].to_vec();
    diagnostic.remap_artifact_paths(|path| (path == &private).then(|| retained.clone()));
    assert_eq!(
        diagnostic.origin(),
        &origin("/inputs/consumer.slib", "lir/initialization/role")
    );
    assert_eq!(
        diagnostic.notes()[0].origin(),
        &origin("/inputs/consumer.slib", "mir/callable")
    );
    assert_eq!(diagnostic.notes()[0].message(), "same artifact");
    assert_eq!(&diagnostic.notes()[1..], unchanged_notes);
    assert_eq!(diagnostic.severity(), DiagnosticSeverityV1::Error);
    assert_eq!(diagnostic.code(), "SCOOPC_BUILD_FAILED");
    assert_eq!(diagnostic.message(), "invalid registration");
    let bytes = scoop_wire::encode(&diagnostic).unwrap();
    assert_eq!(
        scoop_wire::decode_canonical::<StructuredDiagnosticV1>(&bytes).unwrap(),
        diagnostic
    );
}
