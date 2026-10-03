//! HIR selected-use and terminal-provider MIR export matching.

use std::collections::BTreeMap;

use scoop_identity::{ConeIdentity, DependencyCallableDeclarationId};
use scoop_mir::ParamFreeMirCallableExportV1;

use super::{CrossConeClosureMirBridgeError, CrossConeMirClosureRelationError};
use crate::{
    MirBridgeValidatedCrossConeHirFrontSections,
    cross_cone_closure::surface_validation::transitive_dependency_positions,
};

pub(super) fn validate_selected_closure(
    artifacts: &[MirBridgeValidatedCrossConeHirFrontSections<'_>],
    positions: &BTreeMap<ConeIdentity, usize>,
    dependency_positions: &[Vec<usize>],
) -> Result<(), CrossConeClosureMirBridgeError> {
    for (position, consumer) in artifacts.iter().enumerate() {
        let reachable = transitive_dependency_positions(position, dependency_positions);
        for selected in consumer.mir_cross_cone_bridge().selected() {
            let provider_position =
                positions
                    .get(&selected.provider())
                    .copied()
                    .ok_or_else(|| CrossConeClosureMirBridgeError::Relation {
                        identity: consumer.identity(),
                        source: Box::new(CrossConeMirClosureRelationError::MissingProvider {
                            provider: selected.provider(),
                            declaration: selected.declaration(),
                        }),
                    })?;
            if reachable.binary_search(&provider_position).is_err() {
                return Err(CrossConeClosureMirBridgeError::Relation {
                    identity: consumer.identity(),
                    source: Box::new(CrossConeMirClosureRelationError::UnreachableProvider {
                        provider: selected.provider(),
                        declaration: selected.declaration(),
                    }),
                });
            }
            let provider = &artifacts[provider_position];
            let export = find_export(
                provider.mir_cross_cone_bridge().exports(),
                selected.declaration(),
            )
            .ok_or_else(|| CrossConeClosureMirBridgeError::Relation {
                identity: consumer.identity(),
                source: Box::new(CrossConeMirClosureRelationError::MissingProviderExport {
                    provider: selected.provider(),
                    declaration: selected.declaration(),
                }),
            })?;
            if selected.implementation() != export.implementation() {
                return Err(CrossConeClosureMirBridgeError::Relation {
                    identity: consumer.identity(),
                    source: Box::new(
                        CrossConeMirClosureRelationError::SelectedImplementationMismatch {
                            provider: selected.provider(),
                            declaration: selected.declaration(),
                            implementations: Box::new(super::MirImplementationMismatch {
                                expected: export.implementation(),
                                actual: selected.implementation(),
                            }),
                        },
                    ),
                });
            }
            if selected.signature() != export.signature() {
                return Err(CrossConeClosureMirBridgeError::Relation {
                    identity: consumer.identity(),
                    source: Box::new(
                        CrossConeMirClosureRelationError::SelectedSignatureMismatch {
                            provider: selected.provider(),
                            declaration: selected.declaration(),
                        },
                    ),
                });
            }
        }
    }
    Ok(())
}

fn find_export(
    exports: &[ParamFreeMirCallableExportV1],
    declaration: DependencyCallableDeclarationId,
) -> Option<&ParamFreeMirCallableExportV1> {
    exports
        .binary_search_by_key(&declaration, ParamFreeMirCallableExportV1::declaration)
        .ok()
        .map(|index| &exports[index])
}
