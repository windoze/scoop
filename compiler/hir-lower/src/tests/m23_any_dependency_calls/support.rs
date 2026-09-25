use std::path::PathBuf;

use scoop_hir as hir;
use scoop_wire::{BudgetMeter, DecodeLimits};

use crate::tests::m23_ordinary_core_only::support::{parsed_ordinary_text, trusted_core};
use crate::tests::m23_ordinary_dependencies::support::{
    certificate, empty_alias_expansions, project_dependency_text,
};
use crate::{CurrentConeSources, lower_current_cone};

pub(super) fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-any-call-signatures")
        .join(name)
}

pub(super) fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

pub(super) fn with_output<T>(
    case: &str,
    inspect: impl FnOnce(Result<hir::DependencyHirOutput, Vec<scoop_ast::Diagnostic>>) -> T,
) -> T {
    let mut core = trusted_core();
    let provider = scoop_identity::ConeCoordinate::new("test", "any-calls", "1.0.0").unwrap();
    let source = std::fs::read_to_string(fixture(
        "../m23-core-layout-exports/property-initialization-provider.scoop",
    ))
    .unwrap();
    let (foundation, interface) =
        project_dependency_text(&core, &provider, &source, &["Int", "String"]);
    let foundation = core.import_dependency_foundation(&provider, &foundation, 97);
    let aliases = empty_alias_expansions();
    let source = std::fs::read_to_string(fixture(&format!("{case}.scoop"))).unwrap();
    let parsed = parsed_ordinary_text(&source);
    let world = hir::ImportedSemanticWorld::from_validated_closure(
        parsed.cone(),
        vec![
            core.provider(),
            hir::DirectImportedProviderInput::from_validated(
                certificate(&provider, 97),
                &foundation,
                &interface,
                &aliases,
            ),
        ],
        vec![],
    )
    .unwrap();
    let protocols = core.foundation.import_core_inputs(&core.interface).unwrap();
    let input = CurrentConeSources::try_new(&parsed, protocols, &world).unwrap();
    inspect(lower_current_cone(
        scoop_identity::RequestedConeKind::Library,
        &input,
    ))
}

pub(super) fn snapshot(name: &str, text: &str) {
    let path = fixture(name);
    if std::env::var_os("SCOOP_UPDATE_ANY_CALLS").is_some() {
        std::fs::write(&path, text).unwrap();
    }
    assert_eq!(text, std::fs::read_to_string(path).unwrap());
}
