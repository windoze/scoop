use super::super::m23_ordinary_core_only::support::{parsed_ordinary_text_at, trusted_core};
use crate::{
    CurrentConeSources, EditingAnalysisError, lower_current_export,
    lower_current_export_with_cancellation,
};
use scoop_identity::{ConeCoordinate, RequestedConeKind};
use std::{cell::Cell, rc::Rc};

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

#[test]
fn cancelled_source_export_is_distinct_from_failure_and_can_be_retried() {
    let core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "cancelled-export", "1.0.0").unwrap();
    let statements = (0..512)
        .map(|index| format!("val v{index}: Boolean = true;\n"))
        .collect::<String>();
    let text = format!(
        "/// The computed [Boolean].\npublic fun value(): Boolean {{ {statements} return true }}"
    );
    let source = parsed_ordinary_text_at(&coordinate, &text);
    let world = core.world(source.cone());
    let input = CurrentConeSources::try_new(
        &source,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    let input = input
        .with_documentation(vec![scoop_ast::IdentifiedSourceDocumentation {
            source: source.iter().next().unwrap().text().identity().clone(),
            documentation: scoop_doc::prepare_source(&scoop_parser::parse(&text).unwrap(), &text),
        }])
        .unwrap();
    let checks = Rc::new(Cell::new(0));
    let measured = checks.clone();
    lower_current_export_with_cancellation(RequestedConeKind::Library, &input, move || {
        measured.set(measured.get() + 1);
        false
    })
    .unwrap();
    let completed_checks = checks.get();
    for stop in [1, completed_checks / 2, completed_checks - 2] {
        checks.set(0);
        let measured = checks.clone();
        let result =
            lower_current_export_with_cancellation(RequestedConeKind::Library, &input, move || {
                measured.set(measured.get() + 1);
                measured.get() == stop
            });
        assert!(matches!(result, Err(EditingAnalysisError::Cancelled)));
        assert_eq!(checks.get(), stop, "a transient cancellation stays latched");
    }
    let retried =
        lower_current_export_with_cancellation(RequestedConeKind::Library, &input, || false)
            .unwrap();
    assert!(
        retried
            .export
            .functions
            .iter()
            .any(|(_, function)| function.name == "value")
    );
    assert!(retried.documentation.is_some());
}
