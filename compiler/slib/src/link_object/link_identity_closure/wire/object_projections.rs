//! Rebuild the physical atom and digest projections without copying the wire.

use super::resources::table;
use super::*;
use scoop_wire::{WirePath, encode_canonical_temporary_with_meter};

impl DecodedLinkIdentityClosureSectionV1 {
    pub fn replay_object_projections(
        &self,
        member_plan: &PlannedLinkObjectMemberSetV1,
        patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), LinkObjectProjectionValidationError> {
        let path = WirePath::root().field(2);
        // Member-plan equality includes each complete typed unit grouping.
        let units = member_plan.definition_assignments().len();
        meter.charge_work(units as u64, &path)?;
        if patch_sites.builtins().member_plan() != member_plan {
            return Err(LinkObjectProjectionValidationError::MemberPlanMismatch);
        }
        let members = patch_sites.builtins().strong_relocations().members();
        meter.charge_work(members.len() as u64, &path)?;
        let mut count = 0_usize;
        for member in members {
            let definitions = member.definitions().definitions();
            meter.charge_work(definitions.len() as u64, &path)?;
            count = count.saturating_add(definitions.len());
            for definition in definitions {
                table::<super::super::VerifiedDefinitionAtomRangeProjectionV1>(
                    definition.atoms().len(),
                    meter,
                    &path,
                )?;
            }
        }
        table::<super::super::VerifiedObjectDefinitionIndexV1>(count, meter, &path)?;
        validate_array_projection(
            &self.definition_indexes,
            &super::super::definition_indexes(patch_sites.builtins()),
            meter,
            &path,
        )?;
        validate_array_projection(
            &self.patch_sites,
            patch_sites.sites(),
            meter,
            &WirePath::root().field(3),
        )
    }
}

fn validate_array_projection(
    actual: &[impl WireEncode],
    expected: &[impl WireEncode],
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), LinkObjectProjectionValidationError> {
    let actual = encode_canonical_temporary_with_meter(&WireArray(actual), meter, path)?;
    let expected = encode_canonical_temporary_with_meter(&WireArray(expected), meter, path)?;
    meter.charge_work(
        (actual.len() as u64)
            .saturating_add(expected.len() as u64)
            .saturating_mul(3),
        path,
    )?;
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
