//! Final production data assembled without rewriting object bytes.

use super::EntryProductionDataV1;
use crate::link_object::{FinalScoopLirObjectV1, RuntimeImageContentV1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalRuntimeObjectsV1<
    D = scoop_lir::StrongTypeDescriptorRefV1,
    C = scoop_lir::StrongTypeDispatchCallableRefV1,
    I = scoop_identity::PersistentInitializationUnitId,
> {
    runtime_image: RuntimeImageContentV1<D, C, I>,
    entry: EntryProductionDataV1,
}

pub type FinalRuntimeObjectsV2 = FinalRuntimeObjectsV1<
    scoop_lir::StrongTypeDescriptorRefV2,
    scoop_lir::StrongTypeDispatchCallableRefV2,
    scoop_lir::StrongInitializationDependencyRefV2,
>;

impl<D: scoop_lir::StrongDescriptorReference, C: Clone, I> FinalRuntimeObjectsV1<D, C, I> {
    pub fn new(
        runtime_image: RuntimeImageContentV1<D, C, I>,
        entry: EntryProductionDataV1,
    ) -> Self {
        Self {
            runtime_image,
            entry,
        }
    }
    pub const fn runtime_image(&self) -> &RuntimeImageContentV1<D, C, I> {
        &self.runtime_image
    }
    pub const fn entry(&self) -> &EntryProductionDataV1 {
        &self.entry
    }
    pub fn objects(&self) -> &[FinalScoopLirObjectV1] {
        self.runtime_image.registrations().objects()
    }
}
