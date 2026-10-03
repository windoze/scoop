use scoop_protocol::{HostPathCarrier, HostPathError, StructuredDiagnosticV1};

use crate::CompletedNode;

pub(super) fn restore_artifact_locations(
    diagnostics: &mut [StructuredDiagnosticV1],
    completed: &[&CompletedNode],
) -> Result<(), HostPathError> {
    let locations = completed
        .iter()
        .map(|node| {
            Ok((
                HostPathCarrier::from_path(node.materialized_child_path().as_path())?,
                HostPathCarrier::from_path(node.artifact_locator())?,
            ))
        })
        .collect::<Result<Vec<_>, HostPathError>>()?;
    for diagnostic in diagnostics {
        diagnostic.remap_artifact_paths(|path| {
            locations
                .iter()
                .find_map(|(private, original)| (path == private).then(|| original.clone()))
        });
    }
    Ok(())
}
