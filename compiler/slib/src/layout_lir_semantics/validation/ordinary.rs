use std::collections::BTreeMap;

use scoop_hir::OdrFreeHirFoundation;
use scoop_identity::{ConeCoordinate, ConeIdentity, ValidatedIdentityGraph};
use scoop_lir::{
    CrossConeLirBridgeSectionV1, DecodedCrossConeLayoutAbiSectionV1,
    DecodedStrongProductionSectionV2, OdrFreeLirFoundation, ValidatedLirTargetSelection,
};
use scoop_wire::BudgetMeter;

use crate::{
    AbiExpectation, CheckedCrossConeLayoutMirProviderV1, CrossConeLayoutLirSemanticClosureError,
    CrossConeLirClosureRelationError, PreparedCrossConeLayoutLirValidation,
    replay_canonical_scoop_abi_parts, transitive_positions, validate_local_projection,
};

pub(super) struct PreparedLayoutLirFront<'a> {
    pub(super) position: usize,
    pub(super) provider: ConeIdentity,
    pub(super) coordinate: ConeCoordinate,
    pub(super) identities: &'a mut ValidatedIdentityGraph,
    pub(super) hir_foundation: &'a OdrFreeHirFoundation,
    pub(super) foundation: &'a OdrFreeLirFoundation,
    pub(super) meter: &'a mut BudgetMeter,
    pub(super) mir: &'a CheckedCrossConeLayoutMirProviderV1<'a>,
    pub(super) ordinary: CrossConeLirBridgeSectionV1,
    pub(super) strong: DecodedStrongProductionSectionV2,
    pub(super) layout: DecodedCrossConeLayoutAbiSectionV1,
}

pub(super) fn validate<'a, HE, ME, LE>(
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    validations: Vec<PreparedCrossConeLayoutLirValidation<'a>>,
    mir: &[&'a CheckedCrossConeLayoutMirProviderV1<'a>],
    positions: &BTreeMap<ConeIdentity, usize>,
    dependency_positions: &[Vec<usize>],
) -> Result<Vec<PreparedLayoutLirFront<'a>>, CrossConeLayoutLirSemanticClosureError<HE, ME, LE>> {
    let count = validations.len();
    let mut fronts = Vec::new();
    let mut abi_expectations = Vec::new();
    fronts.try_reserve_exact(count).map_err(|_| {
        CrossConeLayoutLirSemanticClosureError::Allocation {
            requested_slots: count,
        }
    })?;
    for (position, (validation, mir)) in validations.into_iter().zip(mir).enumerate() {
        let provider = mir.provider();
        let candidates = validation.candidates;
        let ordinary = candidates
            .ordinary
            .validate(validation.identities, validation.foundation)
            .map_err(
                |source| CrossConeLayoutLirSemanticClosureError::OrdinaryBridge {
                    provider,
                    source: Box::new(source),
                },
            )?;
        validate_local_projection(
            provider,
            mir.hir().public().section(),
            mir.ordinary_bridge(),
            &ordinary,
            &mut abi_expectations,
        )
        .map_err(|source| CrossConeLayoutLirSemanticClosureError::Relation {
            provider,
            source: Box::new(source),
        })?;
        fronts.push(PreparedLayoutLirFront {
            position,
            provider,
            coordinate: validation.coordinate,
            identities: validation.identities,
            hir_foundation: validation.hir_foundation,
            foundation: validation.foundation,
            meter: validation.meter,
            mir,
            ordinary,
            strong: candidates.strong,
            layout: candidates.layout,
        });
    }
    validate_canonical_abis(current, target, &mut fronts, positions, abi_expectations)?;
    validate_terminal_selections(&fronts, positions, dependency_positions)?;
    Ok(fronts)
}

fn validate_canonical_abis<HE, ME, LE>(
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    fronts: &mut [PreparedLayoutLirFront<'_>],
    positions: &BTreeMap<ConeIdentity, usize>,
    expectations: Vec<AbiExpectation>,
) -> Result<(), CrossConeLayoutLirSemanticClosureError<HE, ME, LE>> {
    if current == ConeIdentity::CORE {
        return Ok(());
    }
    let core_position = positions
        .get(&ConeIdentity::CORE)
        .copied()
        .ok_or(CrossConeLayoutLirSemanticClosureError::MissingTrustedCore)?;
    let core = &mut fronts[core_position];
    for expectation in expectations {
        let expected = replay_canonical_scoop_abi_parts(
            target.target(),
            core.meter,
            core.identities,
            core.hir_foundation,
            &expectation.signature,
            expectation.gc_effect,
        )
        .map_err(|source| CrossConeLayoutLirSemanticClosureError::AbiReplay {
            provider: expectation.artifact,
            declaration: expectation.declaration,
            source: Box::new(source),
        })?;
        if expected != expectation.actual {
            return Err(CrossConeLayoutLirSemanticClosureError::Relation {
                provider: expectation.artifact,
                source: Box::new(CrossConeLirClosureRelationError::NonCanonicalExportAbi {
                    declaration: expectation.declaration,
                }),
            });
        }
    }
    Ok(())
}

fn validate_terminal_selections<HE, ME, LE>(
    fronts: &[PreparedLayoutLirFront<'_>],
    positions: &BTreeMap<ConeIdentity, usize>,
    dependency_positions: &[Vec<usize>],
) -> Result<(), CrossConeLayoutLirSemanticClosureError<HE, ME, LE>> {
    for (position, front) in fronts.iter().enumerate() {
        let reachable = transitive_positions(position, dependency_positions);
        for selected in front.ordinary.selected() {
            let provider = selected.provider();
            let declaration = selected.bridge().declaration();
            let terminal_position = positions.get(&provider).copied().ok_or_else(|| {
                relation(
                    front.provider,
                    CrossConeLirClosureRelationError::MissingProvider {
                        provider,
                        declaration,
                    },
                )
            })?;
            if reachable.binary_search(&terminal_position).is_err() {
                return Err(relation(
                    front.provider,
                    CrossConeLirClosureRelationError::UnreachableProvider {
                        provider,
                        declaration,
                    },
                ));
            }
            let export = fronts[terminal_position]
                .ordinary
                .export(declaration)
                .ok_or_else(|| {
                    relation(
                        front.provider,
                        CrossConeLirClosureRelationError::MissingProviderExport {
                            provider,
                            declaration,
                        },
                    )
                })?;
            if selected.bridge() != export {
                return Err(relation(
                    front.provider,
                    CrossConeLirClosureRelationError::SelectedProviderExportMismatch {
                        provider,
                        declaration,
                    },
                ));
            }
        }
    }
    Ok(())
}

fn relation<HE, ME, LE>(
    provider: ConeIdentity,
    source: CrossConeLirClosureRelationError,
) -> CrossConeLayoutLirSemanticClosureError<HE, ME, LE> {
    CrossConeLayoutLirSemanticClosureError::Relation {
        provider,
        source: Box::new(source),
    }
}
