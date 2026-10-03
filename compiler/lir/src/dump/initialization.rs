use std::fmt::Write;

use crate::{
    StrongInitializationDependencyKindV2 as Kind, StrongInitializationUnitRegistrationPlanSetV2,
};

/// Supplements local arena edges with the complete production dependency list
/// for units that also depend on another Cone's initialization.
pub fn dump_initialization_dependencies(
    units: &StrongInitializationUnitRegistrationPlanSetV2,
) -> String {
    let mut text = String::new();
    for registration in units.registrations() {
        let unit = registration.semantic();
        if !unit
            .dependencies()
            .iter()
            .any(|dependency| matches!(dependency.kind(), Kind::DependencyExternalUnit { .. }))
        {
            continue;
        }
        let dependencies = unit
            .dependencies()
            .iter()
            .map(|dependency| {
                let role = match dependency.kind() {
                    Kind::LocalUnit(_) => "local",
                    Kind::DependencyExternalUnit { .. } => "external",
                };
                format!(
                    "{role}(provider={},unit={})",
                    dependency.definition().provider(),
                    dependency.unit()
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(text, "  init-dependencies {} [{dependencies}]", unit.unit())
            .expect("writing to String");
    }
    text
}
