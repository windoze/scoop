//! Owned LIR candidates kept outside the HIR/MIR proof arenas.

use scoop_identity::{ConeCoordinate, ValidatedIdentityGraph};
use scoop_lir::{
    DecodedCrossConeLayoutAbiSectionV1, DecodedCrossConeLirBridgeSectionV1,
    DecodedStrongProductionSectionV2, OdrFreeLirFoundation,
};
use scoop_wire::BudgetMeter;

pub(crate) struct DecodedCrossConeLayoutLirCandidates {
    pub(crate) strong: DecodedStrongProductionSectionV2,
    pub(crate) ordinary: DecodedCrossConeLirBridgeSectionV1,
    pub(crate) layout: DecodedCrossConeLayoutAbiSectionV1,
}

/// Inputs for the next scoped proof layer. The mutable graph and budget are
/// borrowed from the same artifact whose checked HIR/MIR tokens accompany
/// this value; candidates remain owned and can only be consumed once.
pub(crate) struct PreparedCrossConeLayoutLirValidation<'a> {
    pub(crate) coordinate: ConeCoordinate,
    pub(crate) identities: &'a mut ValidatedIdentityGraph,
    pub(crate) foundation: &'a OdrFreeLirFoundation,
    pub(crate) meter: &'a mut BudgetMeter,
    pub(crate) candidates: DecodedCrossConeLayoutLirCandidates,
}
