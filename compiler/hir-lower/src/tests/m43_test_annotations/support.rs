use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{
    ConeCoordinate, NormalizedSourcePath, RequestedConeKind, SourceIdentity, SourceRole,
};

use crate::tests::m23_ordinary_core_only::support::{TrustedCoreFixture, trusted_core_from_source};
use crate::{CurrentConeSources, lower_current_cone};

pub(super) const MARKERS: &str =
    include_str!("../../../../../sysroot/lib/scoop.test/src/markers.scoop");

pub(super) fn marker_core() -> TrustedCoreFixture {
    let mut source = crate::tests::complete_core_file();
    let annotations =
        include_str!("../../../../../sysroot/lib/scoop.core/src/serialization_annotations.scoop");
    source
        .declarations
        .extend(scoop_parser::parse(annotations).unwrap().declarations);
    trusted_core_from_source(source, annotations)
}

pub(super) fn lower(
    text: &str,
    role: SourceRole,
) -> Result<hir::DependencyHirOutput, Vec<ast::Diagnostic>> {
    lower_files(&[("tests/check.scoop", text, role)])
}

pub(super) fn lower_files(
    files: &[(&str, &str, SourceRole)],
) -> Result<hir::DependencyHirOutput, Vec<ast::Diagnostic>> {
    lower_mode(files, false)
}

pub(super) fn lower_tests(
    files: &[(&str, &str, SourceRole)],
) -> Result<hir::DependencyHirOutput, Vec<ast::Diagnostic>> {
    lower_mode(files, true)
}

fn lower_mode(
    files: &[(&str, &str, SourceRole)],
    tests: bool,
) -> Result<hir::DependencyHirOutput, Vec<ast::Diagnostic>> {
    let sources = parsed_sources(files, tests);
    let cone = sources.cone();
    let core = marker_core();
    let world = core.world(sources.cone());
    let input = CurrentConeSources::try_new(
        &sources,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap()
    .with_source_names(std::collections::BTreeMap::from([(
        cone,
        "scoop:scoop.test:0.1.0".into(),
    )]));
    if tests {
        crate::lower_current_tests(&input)
    } else {
        lower_current_cone(RequestedConeKind::Library, &input)
    }
}

pub(super) fn parsed_sources(
    files: &[(&str, &str, SourceRole)],
    tests: bool,
) -> ast::CurrentConeParsedSources {
    let mut files = files.to_vec();
    files.push(("src/markers.scoop", MARKERS, SourceRole::Production));
    if tests {
        files.extend([
            (
                "src/execution/data.scoop",
                include_str!("../../../../../sysroot/lib/scoop.test/src/execution/data.scoop"),
                SourceRole::Production,
            ),
            (
                "src/execution/run.scoop",
                include_str!("../../../../../sysroot/lib/scoop.test/src/execution/run.scoop"),
                SourceRole::Production,
            ),
            (
                "src/failure.scoop",
                include_str!("../../../../../sysroot/lib/scoop.test/src/failure.scoop"),
                SourceRole::Production,
            ),
        ]);
    }
    files.sort_by_key(|(path, _, _)| *path);
    let cone = ConeCoordinate::new("scoop", "scoop.test", "0.1.0")
        .unwrap()
        .identity()
        .unwrap();
    let identity =
        |path| SourceIdentity::new(cone, NormalizedSourcePath::new(path).unwrap()).unwrap();
    let identities = files
        .iter()
        .map(|(path, _, _)| identity(path))
        .collect::<Vec<_>>();
    let mut parsed_inputs = files
        .iter()
        .zip(&identities)
        .map(|((path, text, role), identity)| {
            scoop_parser::CurrentConeSourceInput::new(identity, text, path.as_ref())
                .with_role(*role)
        })
        .collect::<Vec<_>>();
    let first = parsed_inputs.remove(0);
    scoop_parser::parse_current_cone(ast::NonEmptyVec::new(first, parsed_inputs)).unwrap()
}
