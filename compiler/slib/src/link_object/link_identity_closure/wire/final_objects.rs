//! Borrow the final directory and owner projections without promoting wire.

use super::*;
use crate::VerifiedCodeLinkObjectMemberSetV1;
use scoop_wire::{WirePath, encode_canonical_temporary_with_meter};

impl DecodedLinkIdentityClosureSectionV1 {
    pub fn replay_final_object_projections<D, C, I>(
        &self,
        objects: &VerifiedCodeLinkObjectMemberSetV1<D, C, I>,
        meter: &mut BudgetMeter,
    ) -> Result<(), LinkFinalObjectProjectionError>
    where
        D: scoop_lir::StrongDescriptorReference,
        C: Clone,
    {
        meter.charge_work(2, &WirePath::root())?;
        let (image, entry) = super::super::final_objects::owners(objects)
            .map_err(LinkFinalObjectProjectionError::Expected)?;
        same(&self.verified_link_objects, objects.projection(), 6, meter)?;
        same(&self.image_owner, &image, 7, meter)?;
        same(&self.entry_owner, &entry, 8, meter)
    }
}

fn same(
    actual: &impl WireEncode,
    expected: &impl WireEncode,
    field: u32,
    meter: &mut BudgetMeter,
) -> Result<(), LinkFinalObjectProjectionError> {
    let path = WirePath::root().field(field);
    let actual = encode_canonical_temporary_with_meter(actual, meter, &path)?;
    let expected = encode_canonical_temporary_with_meter(expected, meter, &path)?;
    meter.charge_work(actual.len().min(expected.len()) as u64, &path)?;
    if actual != expected {
        return Err(LinkFinalObjectProjectionError::FieldMismatch { field });
    }
    Ok(())
}

#[derive(Debug)]
pub enum LinkFinalObjectProjectionError {
    Resource(WireError),
    Expected(LinkIdentityClosureBuildError),
    FieldMismatch { field: u32 },
}

impl From<WireError> for LinkFinalObjectProjectionError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for LinkFinalObjectProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid final Link object projection: {self:?}")
    }
}

impl std::error::Error for LinkFinalObjectProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Expected(error) => Some(error),
            Self::FieldMismatch { .. } => None,
        }
    }
}
