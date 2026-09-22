use super::*;
use scoop_identity::{PendingIdentityValidation, ValidatedIdentityGraph};

pub(super) fn lower_minimal() -> hir::Output {
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/core-source-foundation.scoop"
    ));
    lower_extra(text)
}

pub(in crate::tests::m23_type_semantics_production::source_binding) fn lower_extra(
    text: &str,
) -> hir::Output {
    lower(vec![
        (
            "src/core.scoop".to_owned(),
            String::new(),
            complete_core_file(),
        ),
        (
            "src/core-source-foundation.scoop".to_owned(),
            text.to_owned(),
            scoop_parser::parse(text).unwrap(),
        ),
    ])
}

pub(super) fn lower_protocol_core() -> hir::Output {
    let parsed = crate::tests::m23_ordinary_core_only::support::parsed_core(complete_core_file());
    lower_core_bootstrap(&CoreBootstrapSources::try_new(&parsed).unwrap()).unwrap()
}
pub(super) fn lower_sysroot() -> hir::Output {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sysroot/lib/scoop.core/src");
    let mut paths = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|s| s == "scoop"))
        .collect::<Vec<_>>();
    paths.sort();
    lower(
        paths
            .into_iter()
            .map(|p| {
                let text = std::fs::read_to_string(&p).unwrap();
                let ast = scoop_parser::parse(&text).unwrap();
                (
                    format!("src/{}", p.file_name().unwrap().to_str().unwrap()),
                    text,
                    ast,
                )
            })
            .collect(),
    )
}
fn lower(mut files: Vec<(String, String, ast::SourceFile)>) -> hir::Output {
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut parsed = Vec::new();
    let mut texts = Vec::new();
    let mut diagnostics = Vec::new();
    for (path, text, file) in files {
        let id = crate::tests::core_source_identity(&path);
        parsed.push(ast::IdentifiedParsedSource::new(id.clone(), file));
        texts.push(ast::CurrentSourceText::new(id.clone(), text));
        diagnostics.push(ast::CurrentSourceDiagnosticContext::new(id, path.into()));
    }
    let parsed = ast::CurrentConeParsedSources::try_new(
        ast::AllParsedSources::try_new(ast::NonEmptyVec::new(parsed.remove(0), parsed)).unwrap(),
        ast::NonEmptyVec::new(texts.remove(0), texts),
        ast::NonEmptyVec::new(diagnostics.remove(0), diagnostics),
    )
    .unwrap();
    lower_core_bootstrap(&CoreBootstrapSources::try_new(&parsed).unwrap()).unwrap()
}
pub(in crate::tests::m23_type_semantics_production::source_binding) fn artifact(
    output: &hir::Output,
) -> (hir::OdrFreeHirFoundation, ValidatedIdentityGraph) {
    let foundation = hir::CanonicalHirFoundation::from_modules(
        &output.export,
        &output.local,
        &output.native_boundary_types,
    )
    .unwrap();
    let decoded: hir::DecodedHirFoundation =
        decode_canonical(&encode(&foundation).unwrap(), DecodeLimits::default()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let mut ids = pending.finish().unwrap();
    let foundation = decoded
        .validate(
            &scoop_identity::ConeCoordinate::reserved_core(),
            &mut ids,
            &mut meter(),
        )
        .unwrap();
    (
        hir::OdrFreeHirFoundation::from_validated(foundation).unwrap(),
        ids,
    )
}
pub(in crate::tests::m23_type_semantics_production::source_binding) fn import(
    foundation: &hir::OdrFreeHirFoundation,
    identities: &ValidatedIdentityGraph,
) -> hir::ImportedHirFoundation {
    let mut session = scoop_identity::SemanticIdentitySession::new();
    let imported = session
        .import(
            ConeIdentity::CORE,
            scoop_identity::SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]),
            identities,
        )
        .unwrap();
    let (hir, _, _) = imported.into_parts();
    hir::ImportedHirFoundation::from_odr_free(foundation.clone(), hir)
}
