//! HIR selected-use and terminal-provider MIR export matching.

use std::collections::BTreeMap;

use scoop_hir::{CrossConeHirInterfaceSectionV1, ExternalHirReferenceRoleV1, ExternalHirTargetV1};
use scoop_identity::{CallableTemplateOrigin, ConeIdentity, DependencyCallableDeclarationId};
use scoop_mir::{CrossConeMirBridgeSectionV1, ParamFreeMirCallableExportV1};

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
        validate_hir_selected_set(consumer.hir_interface(), consumer.mir_cross_cone_bridge())
            .map_err(|source| CrossConeClosureMirBridgeError::Relation {
                identity: consumer.identity(),
                source: Box::new(source),
            })?;
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

fn validate_hir_selected_set(
    interface: &CrossConeHirInterfaceSectionV1,
    bridge: &CrossConeMirBridgeSectionV1,
) -> Result<(), CrossConeMirClosureRelationError> {
    let selected = bridge.selected();
    for record in selected {
        let target = dependency_hir_target(record.declaration());
        let reference = interface.external_references().get(target).ok_or(
            CrossConeMirClosureRelationError::MissingHirSelection {
                declaration: record.declaration(),
            },
        )?;
        if reference.origin() != record.provider() {
            return Err(
                CrossConeMirClosureRelationError::HirSelectionOriginMismatch {
                    declaration: record.declaration(),
                    expected: record.provider(),
                    actual: reference.origin(),
                },
            );
        }
        if !reference
            .roles()
            .contains(ExternalHirReferenceRoleV1::ConcreteSelectedUse)
        {
            return Err(CrossConeMirClosureRelationError::MissingHirSelectionRole {
                declaration: record.declaration(),
            });
        }
    }

    for reference in interface.external_references().records() {
        if !reference
            .roles()
            .contains(ExternalHirReferenceRoleV1::ConcreteSelectedUse)
        {
            continue;
        }
        let declaration = match reference.target() {
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::Function(declaration)) => {
                Some(DependencyCallableDeclarationId::Function(declaration))
            }
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::Accessor(declaration)) => Some(
                DependencyCallableDeclarationId::PropertyAccessor(declaration),
            ),
            target @ (ExternalHirTargetV1::Callable(_)
            | ExternalHirTargetV1::GeneratedCallable(_)) => {
                return Err(CrossConeMirClosureRelationError::UnsupportedHirSelection { target });
            }
            ExternalHirTargetV1::Nominal(_)
            | ExternalHirTargetV1::Property(_)
            | ExternalHirTargetV1::ObjectValue(_)
            | ExternalHirTargetV1::TypeAlias(_)
            | ExternalHirTargetV1::Field(_)
            | ExternalHirTargetV1::EnumVariantField(_) => None,
        };
        let Some(declaration) = declaration else {
            continue;
        };
        if selected
            .binary_search_by_key(&(reference.origin(), declaration), |record| {
                (record.provider(), record.declaration())
            })
            .is_err()
        {
            return Err(CrossConeMirClosureRelationError::MissingMirSelection {
                provider: reference.origin(),
                declaration,
            });
        }
    }
    Ok(())
}

fn dependency_hir_target(declaration: DependencyCallableDeclarationId) -> ExternalHirTargetV1 {
    let callable = match declaration {
        DependencyCallableDeclarationId::Function(declaration) => {
            CallableTemplateOrigin::Function(declaration)
        }
        DependencyCallableDeclarationId::PropertyAccessor(declaration) => {
            CallableTemplateOrigin::Accessor(declaration)
        }
    };
    ExternalHirTargetV1::Callable(callable)
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

#[cfg(test)]
mod tests;
