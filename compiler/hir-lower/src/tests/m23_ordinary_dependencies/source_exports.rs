use super::super::m23_ordinary_core_only::support::{parsed_ordinary_text_at, trusted_core};
use crate::{CurrentConeSources, lower_current_export};
use scoop_identity::{ConeCoordinate, RequestedConeKind};

#[test]
fn source_export_retains_checked_generic_bodies_defaults_and_dependency_calls() {
    let core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "source-export", "1.0.0").unwrap();
    let source = parsed_ordinary_text_at(
        &coordinate,
        "public fun <Item> identity(value: Item): Item = value\npublic fun choose(flag: Boolean = true): Boolean = identity(flag)\npublic fun same(flag: Boolean): Boolean = flag.equals(true)",
    );
    let world = core.world(source.cone());
    let input = CurrentConeSources::try_new(
        &source,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    let output = lower_current_export(RequestedConeKind::Library, &input).unwrap();
    assert_eq!(output.export.cone, source.cone());
    assert_eq!(output.imported_dependencies.consumer(), source.cone());
    assert_eq!(output.export.generic_functions.len(), 1);
    assert!(!output.export.export_default_exprs.is_empty());
    assert!(
        output
            .imported_dependencies
            .callables()
            .any(|callable| { callable.provider() == scoop_identity::ConeIdentity::CORE })
    );
    assert!(
        output
            .export
            .functions
            .iter()
            .filter(|(_, function)| {
                matches!(function.name.as_str(), "identity" | "choose" | "same")
            })
            .all(|(_, function)| matches!(function.kind, scoop_hir::FunctionKind::User(_)))
    );
}

#[test]
fn source_export_failure_keeps_real_source_diagnostics() {
    let core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "invalid-export", "1.0.0").unwrap();
    let source = parsed_ordinary_text_at(&coordinate, "public fun value(): Boolean = missing");
    let world = core.world(source.cone());
    let input = CurrentConeSources::try_new(
        &source,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    let errors = lower_current_export(RequestedConeKind::Library, &input)
        .err()
        .unwrap();
    assert!(errors.iter().any(|error| error.message.contains("missing")));
    assert!(errors.iter().all(|error| {
        error
            .source
            .as_ref()
            .is_some_and(|identity| identity.cone() == source.cone())
    }));
}
