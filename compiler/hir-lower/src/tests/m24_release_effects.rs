use scoop_hir as hir;

use super::m23_ordinary_core_only::support::{parsed_ordinary_text, trusted_core};
use crate::{CurrentConeSources, lower_current_cone};

mod calls;
mod metadata;
mod values;

fn with_source(source: &str, verify: impl FnOnce(&hir::ExportHir)) {
    let core = trusted_core();
    let parsed = parsed_ordinary_text(source);
    let world = core.world(parsed.cone());
    let input = CurrentConeSources::try_new(
        &parsed,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, &input)
        .unwrap_or_else(|errors| {
            let messages = errors
                .iter()
                .map(|error| (&error.span, &error.message))
                .collect::<Vec<_>>();
            panic!("{messages:#?}\n{source}")
        });
    verify(output.output().export.module());
}

fn function<'a>(export: &'a hir::ExportHir, name: &str) -> &'a hir::Function {
    export
        .functions
        .values()
        .find(|function| function.name == name)
        .unwrap()
}

fn available(export: &hir::ExportHir, name: &str, parameters: &[&str]) {
    let function = function(export, name);
    let hir::ReleaseCallability::NoTransition { requirements } = &function.release_callability
    else {
        panic!("{name} must be callable from release");
    };
    let mut actual = requirements
        .iter()
        .map(|parameter| function.type_param(*parameter).name.as_str())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(actual, parameters, "{name}");
}

fn unavailable(export: &hir::ExportHir, name: &str) {
    assert_eq!(
        function(export, name).release_callability,
        hir::ReleaseCallability::Unavailable,
        "{name}"
    );
}
