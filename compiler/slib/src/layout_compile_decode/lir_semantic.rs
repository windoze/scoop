//! Owned LIR candidates kept outside the HIR/MIR proof arenas.

use scoop_lir::{
    DecodedCrossConeLayoutAbiSectionV1, DecodedCrossConeLirBridgeSectionV1,
    DecodedStrongProductionSectionV2,
};

pub(crate) struct DecodedCrossConeLayoutLirCandidates {
    pub(crate) strong: DecodedStrongProductionSectionV2,
    pub(crate) ordinary: DecodedCrossConeLirBridgeSectionV1,
    pub(crate) layout: DecodedCrossConeLayoutAbiSectionV1,
}
