//! Owned LIR candidates kept outside the HIR/MIR proof arenas.

use scoop_lir::{
    DecodedConeProductionSectionV2, DecodedCrossConeLayoutAbiSectionV1,
    DecodedCrossConeLirBridgeSectionV1,
};

pub(crate) struct DecodedCrossConeLayoutLirCandidates {
    pub(crate) strong: DecodedConeProductionSectionV2,
    pub(crate) ordinary: DecodedCrossConeLirBridgeSectionV1,
    pub(crate) layout: DecodedCrossConeLayoutAbiSectionV1,
}
