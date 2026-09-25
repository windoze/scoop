//! Rebuild the physical atom and digest projections without copying the wire.

use super::*;
use scoop_wire::{WirePath, encode_canonical_temporary};

impl DecodedLinkIdentityClosureSectionV1 {
    pub fn replay_object_projections(
        &self,
        member_plan: &PlannedLinkObjectMemberSetV1,
        patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    ) -> Result<(), LinkObjectProjectionValidationError> {
        let path = WirePath::root().field(2);
        // Member-plan equality includes each complete typed unit grouping.

        if patch_sites.builtins().member_plan() != member_plan {
            return Err(LinkObjectProjectionValidationError::MemberPlanMismatch);
        }
        validate_array_projection(
            &self.definition_indexes,
            &super::super::definition_indexes(patch_sites.builtins()),
            &path,
        )?;
        validate_array_projection(
            &self.patch_sites,
            patch_sites.sites(),
            &WirePath::root().field(3),
        )
    }
}

fn validate_array_projection(
    actual: &[impl WireEncode],
    expected: &[impl WireEncode],

    path: &WirePath,
) -> Result<(), LinkObjectProjectionValidationError> {
    let actual = encode_canonical_temporary(&WireArray(actual), path)?;
    let expected = encode_canonical_temporary(&WireArray(expected), path)?;

    if actual == expected {
        Ok(())
    } else {
        Err(LinkObjectProjectionValidationError::ProjectionMismatch)
    }
}

impl From<WireError> for LinkObjectProjectionValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
