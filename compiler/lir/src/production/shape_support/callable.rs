use scoop_identity::{CallableDefinitionOwner, PersistentCallableBodyId};

use super::*;
use crate::{ExternalStrongShapeSubjectV1, StrongShapeDefinitionRefV1};

impl StrongShapeDefinitionV1<PersistentCallableBodyId> {
    pub(crate) fn from_callable_definition(
        target: CallableDefinitionOwner,
        physical: StrongShapeDefinitionRefV1,
    ) -> Result<Option<Self>, scoop_wire::HashError> {
        if physical.subject() != ExternalStrongShapeSubjectV1::Callable(target) {
            return Ok(None);
        }
        Ok(Some(Self {
            semantic_id: PersistentCallableBodyId::from_key(&target.body_key())?,
            definition_plan: physical.definition(),
            symbol: physical.symbol(),
        }))
    }
}
