//! Exact local projection and terminal-provider checks for LIR bridges.

use std::collections::BTreeMap;

use scoop_hir::{CallableDeclarationRecordV1, CrossConeHirInterfaceSectionV1};
use scoop_identity::{
    CallableTemplateOrigin, CanonicalScoopAbiFunctionSignature, ConeIdentity,
    DependencyCallableDeclarationId, ExactCallableSignature, GcEffect,
};
use scoop_lir::{CallingConvention, CrossConeLirBridgeSectionV1, SelectedDependencyLirCallableV1};
use scoop_mir::{
    CrossConeMirBridgeSectionV1, ParamFreeMirCallableExportV1, SelectedDependencyMirCallableV1,
};

use super::{CrossConeClosureLirBridgeError, CrossConeLirClosureRelationError};
use crate::{
    LirBridgeValidatedCrossConeHirFrontSections,
    cross_cone_closure::surface_validation::transitive_dependency_positions,
};

pub(super) fn validate_lir_bridge_relations(
    artifacts: &mut [LirBridgeValidatedCrossConeHirFrontSections<'_>],
    positions: &BTreeMap<ConeIdentity, usize>,
    dependency_positions: &[Vec<usize>],
) -> Result<(), CrossConeClosureLirBridgeError> {
    let mut abi_expectations = Vec::new();
    for artifact in artifacts.iter_mut() {
        artifact
            .append_abi_expectations(&mut abi_expectations)
            .map_err(|source| relation(artifact, source))?;
    }
    for expectation in abi_expectations {
        let position = positions
            .get(&expectation.artifact)
            .copied()
            .ok_or_else(|| CrossConeClosureLirBridgeError::Relation {
                identity: expectation.artifact,
                source: Box::new(CrossConeLirClosureRelationError::MissingProvider {
                    provider: expectation.artifact,
                    declaration: expectation.declaration,
                }),
            })?;
        let reachable = transitive_dependency_positions(position, dependency_positions);
        let (before, remaining) = artifacts.split_at_mut(position);
        let (artifact, after) = remaining.split_first_mut().ok_or_else(|| {
            CrossConeClosureLirBridgeError::Relation {
                identity: expectation.artifact,
                source: Box::new(CrossConeLirClosureRelationError::MissingProvider {
                    provider: expectation.artifact,
                    declaration: expectation.declaration,
                }),
            }
        })?;
        let dependencies = reachable.iter().map(|dependency| {
            if *dependency < position {
                before[*dependency].abi_replay_types()
            } else {
                after[*dependency - position - 1].abi_replay_types()
            }
        });
        let expected = artifact
            .replay_canonical_scoop_abi(dependencies, &expectation.signature, expectation.gc_effect)
            .map_err(|source| CrossConeClosureLirBridgeError::AbiReplay {
                identity: expectation.artifact,
                declaration: expectation.declaration,
                source: Box::new(source),
            })?;
        expectation.check_canonical(&expected).map_err(|source| {
            CrossConeClosureLirBridgeError::Relation {
                identity: expectation.artifact,
                source: Box::new(source),
            }
        })?;
    }

    for (position, artifact) in artifacts.iter().enumerate() {
        let reachable = transitive_dependency_positions(position, dependency_positions);
        for selected in artifact.lir_cross_cone_bridge().selected() {
            validate_terminal_provider(selected, artifacts, positions, &reachable)
                .map_err(|source| relation(artifact, source))?;
        }
    }
    Ok(())
}

pub(crate) fn validate_local_projection(
    artifact: ConeIdentity,
    interface: &CrossConeHirInterfaceSectionV1,
    mir: &CrossConeMirBridgeSectionV1,
    lir: &CrossConeLirBridgeSectionV1,
    abi_expectations: &mut Vec<AbiExpectation>,
) -> Result<(), CrossConeLirClosureRelationError> {
    for expected in mir.exports() {
        let declaration = expected.declaration();
        let Some(actual) = lir.export(declaration) else {
            return Err(CrossConeLirClosureRelationError::MissingLirExport { declaration });
        };
        if actual.target() != expected.implementation() {
            return Err(CrossConeLirClosureRelationError::ExportTargetMismatch { declaration });
        }
        if actual.abi_signature().signature() != expected.signature() {
            return Err(
                CrossConeLirClosureRelationError::ExportExactSignatureMismatch { declaration },
            );
        }
        let callable = callable_interface(interface, declaration).ok_or(
            CrossConeLirClosureRelationError::MissingExportCallableInterface { declaration },
        )?;
        let expected_gc = callable.effects().gc_effect();
        let actual_gc = actual.abi_signature().gc_effect();
        if actual_gc != expected_gc {
            return Err(CrossConeLirClosureRelationError::ExportGcEffectMismatch {
                declaration,
                expected: expected_gc,
                actual: actual_gc,
            });
        }
        if actual.calling_convention() != CallingConvention::Cdecl {
            return Err(
                CrossConeLirClosureRelationError::ExportCallingConventionMismatch { declaration },
            );
        }

        let path = scoop_wire::WirePath::root();

        scoop_wire::allocation::try_reserve(abi_expectations, 1, &path)
            .map_err(CrossConeLirClosureRelationError::Resource)?;
        abi_expectations.push(AbiExpectation {
            artifact,
            declaration,
            signature: expected.signature().clone(),
            gc_effect: expected_gc,
            actual: actual.abi_signature().clone(),
        });
    }
    for actual in lir.exports() {
        if mir_export(mir.exports(), actual.declaration()).is_none() {
            return Err(CrossConeLirClosureRelationError::UnexpectedLirExport {
                declaration: actual.declaration(),
            });
        }
    }

    for expected in mir.selected() {
        let key = (expected.provider(), expected.declaration());
        let Some(actual) = selected_lir(lir.selected(), key) else {
            return Err(CrossConeLirClosureRelationError::MissingLirSelection {
                provider: key.0,
                declaration: key.1,
            });
        };
        if actual.bridge().target() != expected.implementation() {
            return Err(CrossConeLirClosureRelationError::SelectedTargetMismatch {
                provider: key.0,
                declaration: key.1,
            });
        }
        if actual.bridge().abi_signature().signature() != expected.signature() {
            return Err(
                CrossConeLirClosureRelationError::SelectedExactSignatureMismatch {
                    provider: key.0,
                    declaration: key.1,
                },
            );
        }
    }
    for actual in lir.selected() {
        let key = (actual.provider(), actual.bridge().declaration());
        if selected_mir(mir.selected(), key).is_none() {
            return Err(CrossConeLirClosureRelationError::UnexpectedLirSelection {
                provider: key.0,
                declaration: key.1,
            });
        }
    }
    Ok(())
}

pub(crate) struct AbiExpectation {
    pub(crate) artifact: ConeIdentity,
    pub(crate) declaration: DependencyCallableDeclarationId,
    pub(crate) signature: ExactCallableSignature,
    pub(crate) gc_effect: GcEffect,
    pub(crate) actual: CanonicalScoopAbiFunctionSignature,
}

impl AbiExpectation {
    pub(crate) fn check_canonical(
        &self,
        expected: &CanonicalScoopAbiFunctionSignature,
    ) -> Result<(), CrossConeLirClosureRelationError> {
        if expected != &self.actual {
            return Err(CrossConeLirClosureRelationError::NonCanonicalExportAbi {
                declaration: self.declaration,
            });
        }
        Ok(())
    }
}

fn validate_terminal_provider(
    selected: &SelectedDependencyLirCallableV1,
    artifacts: &[LirBridgeValidatedCrossConeHirFrontSections<'_>],
    positions: &BTreeMap<ConeIdentity, usize>,
    reachable: &[usize],
) -> Result<(), CrossConeLirClosureRelationError> {
    let provider = selected.provider();
    let declaration = selected.bridge().declaration();
    let position = positions.get(&provider).copied().ok_or(
        CrossConeLirClosureRelationError::MissingProvider {
            provider,
            declaration,
        },
    )?;
    if reachable.binary_search(&position).is_err() {
        return Err(CrossConeLirClosureRelationError::UnreachableProvider {
            provider,
            declaration,
        });
    }
    let Some(export) = artifacts[position]
        .lir_cross_cone_bridge()
        .export(declaration)
    else {
        return Err(CrossConeLirClosureRelationError::MissingProviderExport {
            provider,
            declaration,
        });
    };
    if selected.bridge() != export {
        return Err(
            CrossConeLirClosureRelationError::SelectedProviderExportMismatch {
                provider,
                declaration,
            },
        );
    }
    Ok(())
}

fn callable_interface(
    interface: &CrossConeHirInterfaceSectionV1,
    declaration: DependencyCallableDeclarationId,
) -> Option<&CallableDeclarationRecordV1> {
    let declaration = match declaration {
        DependencyCallableDeclarationId::Function(id) => CallableTemplateOrigin::Function(id),
        DependencyCallableDeclarationId::PropertyAccessor(id) => {
            CallableTemplateOrigin::Accessor(id)
        }
    };
    interface.callable_interfaces().declaration(declaration)
}

fn selected_lir(
    selected: &[SelectedDependencyLirCallableV1],
    key: (ConeIdentity, DependencyCallableDeclarationId),
) -> Option<&SelectedDependencyLirCallableV1> {
    selected
        .binary_search_by_key(&key, |record| {
            (record.provider(), record.bridge().declaration())
        })
        .ok()
        .map(|index| &selected[index])
}

fn mir_export(
    exports: &[ParamFreeMirCallableExportV1],
    declaration: DependencyCallableDeclarationId,
) -> Option<&ParamFreeMirCallableExportV1> {
    exports
        .binary_search_by_key(&declaration, ParamFreeMirCallableExportV1::declaration)
        .ok()
        .map(|index| &exports[index])
}

fn selected_mir(
    selected: &[SelectedDependencyMirCallableV1],
    key: (ConeIdentity, DependencyCallableDeclarationId),
) -> Option<&SelectedDependencyMirCallableV1> {
    selected
        .binary_search_by_key(&key, |record| (record.provider(), record.declaration()))
        .ok()
        .map(|index| &selected[index])
}

fn relation(
    artifact: &LirBridgeValidatedCrossConeHirFrontSections<'_>,
    source: CrossConeLirClosureRelationError,
) -> CrossConeClosureLirBridgeError {
    CrossConeClosureLirBridgeError::Relation {
        identity: artifact.identity(),
        source: Box::new(source),
    }
}

#[cfg(test)]
mod tests;
