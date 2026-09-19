use scoop_identity::{
    CborIdentityRecord, ExactTypeKey, LayoutKey, PersistentExactTypeId, PersistentLayoutId,
    RepresentationRole,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use crate::{
    ExternalStrongShapeSubjectV1, LirTargetProfile, OdrFreeLirFoundation,
    StrongShapeDefinitionError, StrongShapeDefinitionRefV1, StrongShapeDefinitionV1,
};

/// One canonical exact/layout key and its physical definition in the producer.
/// This proves neither a representation body nor cross-Cone authorization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactLayoutIdentityV1 {
    exact: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
    layout: CborIdentityRecord<PersistentLayoutId, LayoutKey>,
    target: LirTargetProfile,
    physical: StrongShapeDefinitionRefV1,
    definition: StrongShapeDefinitionV1<PersistentLayoutId>,
}

impl ExactLayoutIdentityV1 {
    pub fn from_foundation(
        target: LirTargetProfile,
        exact: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
        role: RepresentationRole,
        foundation: &OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ExactLayoutIdentityError> {
        let path = WirePath::root();
        meter.charge_work(1, &path)?;
        let key = LayoutKey::new(exact.id(), target.wire_id(), role);
        meter.charge_work(foundation.layouts().len() as u64, &path)?;
        let layout = foundation
            .layouts()
            .iter()
            .find(|record| record.key() == &key)
            .ok_or(ExactLayoutIdentityError::MissingLayout(key))?;
        let physical = StrongShapeDefinitionRefV1::from_foundation(
            ExternalStrongShapeSubjectV1::Layout(layout.id()),
            foundation,
            meter,
        )?;
        let definition = StrongShapeDefinitionV1::from_layout_definition(layout.id(), physical)
            .ok_or(ExactLayoutIdentityError::DefinitionSubject)?;
        Ok(Self {
            exact,
            layout: layout.clone(),
            target,
            physical,
            definition,
        })
    }

    pub const fn exact(&self) -> PersistentExactTypeId {
        self.exact.id()
    }
    pub fn exact_key(&self) -> &ExactTypeKey {
        self.exact.key()
    }
    pub const fn layout(&self) -> PersistentLayoutId {
        self.layout.id()
    }
    pub fn layout_key(&self) -> &LayoutKey {
        self.layout.key()
    }
    pub const fn target(&self) -> LirTargetProfile {
        self.target
    }
    pub const fn physical_definition(&self) -> StrongShapeDefinitionRefV1 {
        self.physical
    }
    pub const fn definition(&self) -> StrongShapeDefinitionV1<PersistentLayoutId> {
        self.definition
    }
}

#[derive(Debug)]
pub enum ExactLayoutIdentityError {
    MissingLayout(LayoutKey),
    Definition(StrongShapeDefinitionError),
    DefinitionSubject,
    Resource(WireError),
}

impl From<StrongShapeDefinitionError> for ExactLayoutIdentityError {
    fn from(error: StrongShapeDefinitionError) -> Self {
        Self::Definition(error)
    }
}
impl From<WireError> for ExactLayoutIdentityError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for ExactLayoutIdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid exact layout identity: {self:?}")
    }
}
impl std::error::Error for ExactLayoutIdentityError {}

#[cfg(test)]
mod tests;
