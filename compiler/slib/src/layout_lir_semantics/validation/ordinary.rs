use std::collections::BTreeMap;

use scoop_identity::{ConeCoordinate, ConeIdentity, ValidatedIdentityGraph};
use scoop_lir::{
    CrossConeLirBridgeSectionV1, DecodedCrossConeLayoutAbiSectionV1,
    DecodedStrongProductionSectionV2, OdrFreeLirFoundation,
};
use scoop_wire::BudgetMeter;

use crate::{
    AbiExpectation, CheckedCrossConeLayoutMirProviderV1, CrossConeLayoutLirSemanticClosureError,
    CrossConeLirClosureRelationError, PreparedCrossConeLayoutLirValidation, transitive_positions,
    validate_local_projection,
};

pub(super) struct PreparedLayoutLirFront<'a> {
    pub(super) position: usize,
    pub(super) provider: ConeIdentity,
    pub(super) coordinate: ConeCoordinate,
    pub(super) identities: &'a mut ValidatedIdentityGraph,
    pub(super) foundation: &'a OdrFreeLirFoundation,
    pub(super) meter: &'a mut BudgetMeter,
    pub(super) mir: &'a CheckedCrossConeLayoutMirProviderV1<'a>,
    pub(super) ordinary: CrossConeLirBridgeSectionV1,
    pub(super) abi_expectations: Vec<AbiExpectation>,
    pub(super) strong: DecodedStrongProductionSectionV2,
    pub(super) layout: DecodedCrossConeLayoutAbiSectionV1,
    pub(super) link_sections: Option<&'a crate::DecodedCrossConeLayoutLinkOnlySections>,
}

pub(super) fn validate<'a, HE, ME, LE>(
    validations: Vec<PreparedCrossConeLayoutLirValidation<'a>>,
    mir: &[&'a CheckedCrossConeLayoutMirProviderV1<'a>],
    positions: &BTreeMap<ConeIdentity, usize>,
    dependency_positions: &[Vec<usize>],
) -> Result<Vec<PreparedLayoutLirFront<'a>>, CrossConeLayoutLirSemanticClosureError<HE, ME, LE>> {
    let count = validations.len();
    let mut fronts = Vec::new();
    fronts.try_reserve_exact(count).map_err(|_| {
        CrossConeLayoutLirSemanticClosureError::Allocation {
            requested_slots: count,
        }
    })?;
    for (position, (validation, mir)) in validations.into_iter().zip(mir).enumerate() {
        let provider = mir.provider();
        let mut abi_expectations = Vec::new();
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
            validation.meter,
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
            foundation: validation.foundation,
            meter: validation.meter,
            mir,
            ordinary,
            abi_expectations,
            strong: candidates.strong,
            layout: candidates.layout,
            link_sections: validation.link_sections,
        });
    }
    validate_terminal_selections(&fronts, positions, dependency_positions)?;
    Ok(fronts)
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
