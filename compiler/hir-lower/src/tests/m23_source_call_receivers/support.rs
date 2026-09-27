use std::path::PathBuf;

use scoop_hir as hir;

use crate::tests::m23_ordinary_core_only::support::{parsed_ordinary_text, trusted_core};
use crate::tests::m23_ordinary_dependencies::support::{
    empty_alias_expansions, project_dependency_text,
};
use crate::{CurrentConeSources, lower_current_cone};

pub(super) fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-source-call-receivers")
        .join(name)
}

pub(super) fn with_output<T>(
    case: &str,
    inspect: impl FnOnce(hir::DependencyHirOutput, &hir::ImportedSemanticWorld<'_>) -> T,
) -> T {
    let mut core = trusted_core();
    let provider =
        scoop_identity::ConeCoordinate::new("test", "source-receivers", "1.0.0").unwrap();
    let source = std::fs::read_to_string(fixture("provider.scoop")).unwrap();
    let (foundation, interface) =
        project_dependency_text(&core, &provider, &source, &["Int", "String"]);
    let foundation = core.import_dependency_foundation(&provider, &foundation, 96);
    let aliases = empty_alias_expansions();
    let source = std::fs::read_to_string(fixture(&format!("{case}.scoop"))).unwrap();
    let parsed = parsed_ordinary_text(&source);
    let world = hir::ImportedSemanticWorld::from_dependencies(
        parsed.cone(),
        vec![
            core.provider(),
            hir::ImportedProviderInput {
                foundation: &foundation,
                interface: &interface,
                alias_expansions: &aliases,
            },
        ],
        vec![],
    )
    .unwrap();
    let protocols = core.foundation.import_core_inputs(&core.interface).unwrap();
    let input = CurrentConeSources::try_new(&parsed, protocols, &world).unwrap();
    let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, &input)
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    inspect(output, &world)
}
