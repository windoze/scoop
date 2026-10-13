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
    let core = marker_core();
    let cone = ConeCoordinate::new("scoop", "scoop.test", "0.1.0")
        .unwrap()
        .identity()
        .unwrap();
    let identity =
        |path| SourceIdentity::new(cone, NormalizedSourcePath::new(path).unwrap()).unwrap();
    let marker_identity = identity("src/markers.scoop");
    let test_identity = identity("tests/check.scoop");
    let sources = scoop_parser::parse_current_cone(ast::NonEmptyVec::new(
        scoop_parser::CurrentConeSourceInput::new(
            &marker_identity,
            MARKERS,
            "src/markers.scoop".as_ref(),
        ),
        vec![
            scoop_parser::CurrentConeSourceInput::new(
                &test_identity,
                text,
                "tests/check.scoop".as_ref(),
            )
            .with_role(role),
        ],
    ))
    .unwrap();
    let world = core.world(sources.cone());
    let input = CurrentConeSources::try_new(
        &sources,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    lower_current_cone(RequestedConeKind::Library, &input)
}
