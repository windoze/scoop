use scoop_hir::NominalExactLeafClassifierV1;

use super::super::{CrossConeClosureMirBridgeError, CrossConeMirClosureRelationError};
use crate::{
    MirBridgeValidatedCrossConeHirFrontSections, dependency_reachability::transitive_positions,
};

pub(crate) fn validate_export_surfaces(
    artifacts: &mut [MirBridgeValidatedCrossConeHirFrontSections<'_>],
    dependencies: &[Vec<usize>],
) -> Result<(), CrossConeClosureMirBridgeError> {
    for position in 0..artifacts.len() {
        let (preceding, remaining) = artifacts.split_at_mut(position);
        let artifact = &mut remaining[0];
        let identity = artifact.identity();
        validate_provider(artifact, preceding, position, dependencies).map_err(|source| {
            CrossConeClosureMirBridgeError::Relation {
                identity,
                source: Box::new(source),
            }
        })?;
    }
    Ok(())
}

fn validate_provider(
    artifact: &mut MirBridgeValidatedCrossConeHirFrontSections<'_>,
    preceding: &[MirBridgeValidatedCrossConeHirFrontSections<'_>],
    position: usize,
    dependencies: &[Vec<usize>],
) -> Result<(), CrossConeMirClosureRelationError> {
    let (interface, strong, bridge) = artifact.mir_export_validation_parts();
    let reachable = transitive_positions(position, dependencies)
        .map_err(CrossConeMirClosureRelationError::Resource)?;
    let nominal_records =
        interface
            .nominal_interfaces()
            .records()
            .iter()
            .chain(reachable.iter().flat_map(|dependency| {
                preceding[*dependency]
                    .hir_interface()
                    .nominal_interfaces()
                    .records()
            }));
    let classifier = NominalExactLeafClassifierV1::try_from_nominal_interfaces(nominal_records)
        .map_err(CrossConeMirClosureRelationError::NominalClassifier)?;
    super::validate_export_relation(
        interface.callable_interfaces().records(),
        strong,
        bridge,
        &classifier,
    )
}
