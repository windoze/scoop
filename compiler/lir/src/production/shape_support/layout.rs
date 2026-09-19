use super::*;
use crate::{ExternalStrongShapeSubjectV1, StrongShapeDefinitionRefV1};

impl StrongShapeDefinitionV1<PersistentLayoutId> {
    pub(crate) fn from_layout_definition(
        layout: PersistentLayoutId,
        physical: StrongShapeDefinitionRefV1,
    ) -> Option<Self> {
        (physical.subject() == ExternalStrongShapeSubjectV1::Layout(layout)).then_some(Self {
            semantic_id: layout,
            definition_plan: physical.definition(),
            symbol: physical.symbol(),
        })
    }
}

impl<I: PersistentId> DecodedStrongShapeDefinitionV1<I> {
    pub(crate) fn matches_definition(
        self,
        expected: StrongShapeDefinitionV1<I>,
        meter: &mut scoop_wire::BudgetMeter,
    ) -> Result<bool, WireError> {
        let path = scoop_wire::WirePath::root();
        meter.charge_work(3, &path)?;
        if self.semantic_id.verify(expected.semantic_id()).is_err()
            || self
                .definition_plan
                .verify(expected.definition_plan())
                .is_err()
        {
            return Ok(false);
        }
        // The frozen symbol-request product has no standalone refinement API.
        // Compare its complete canonical fields, never merely their digest.
        let actual = scoop_wire::encode_canonical_temporary_with_meter(&self.symbol, meter, &path)?;
        let expected =
            scoop_wire::encode_canonical_temporary_with_meter(&expected.symbol(), meter, &path)?;
        Ok(actual == expected)
    }
}
