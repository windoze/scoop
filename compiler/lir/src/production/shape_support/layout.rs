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
