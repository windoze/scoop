use std::collections::BTreeSet;

use scoop_identity::{
    CanonicalExactTypeDiagnosticName, ExactTypeDiagnosticGraph, PersistentDispatchTableId,
    PersistentExactTypeId, RepresentationRole,
};
use scoop_wire::WirePath;

use super::*;
use crate::{
    ExternalStrongShapeSubjectV1, InstanceRepresentationKindV1, OdrFreeLirFoundation,
    StrongShapeDefinitionRefV1, StrongShapeDefinitionV1, StrongShapeRegistrationV1,
    StrongTypeDescriptorInlineScanPlanV1, StrongTypeDescriptorSemanticPlanV2,
    StrongTypeRegistrationPlanV2, TypeDescriptorITableDirectoryV1, TypeDescriptorInlineScanV1,
};

mod body;
mod inline;
mod registration;
pub(crate) use body::replay_parts;
pub(crate) use inline::{expected_inline_scan, validate_inline_scan};
pub(crate) use registration::validate_registration_plan;

impl ExactDescriptorExportV1 {
    pub fn replay(
        target: crate::LirTargetProfile,
        layouts: &crate::CanonicalExactLayoutExportsV1,
        registration: &StrongTypeRegistrationPlanV2,
        diagnostics: &impl ExactTypeDiagnosticGraph,
        foundation: &OdrFreeLirFoundation,
    ) -> Result<Self, ExactDescriptorError> {
        validate_registration_plan(registration, layouts, foundation)?;
        let expected_registration = StrongShapeRegistrationV1::from_artifact(
            registration.exact_type(),
            registration.definition_plan(),
            registration.symbol(),
            registration.registration_fingerprint_node(),
        );
        replay_parts(
            target,
            layouts,
            registration.semantic(),
            expected_registration,
            diagnostics,
            foundation,
        )
    }
}

#[cfg(test)]
pub(super) fn replay_test_parts(
    target: crate::LirTargetProfile,
    layouts: &crate::CanonicalExactLayoutExportsV1,
    semantic: &StrongTypeDescriptorSemanticPlanV2,
    registration: StrongShapeRegistrationV1<PersistentExactTypeId>,
    diagnostics: &impl ExactTypeDiagnosticGraph,
    foundation: &OdrFreeLirFoundation,
) -> Result<ExactDescriptorExportV1, ExactDescriptorError> {
    replay_parts(
        target,
        layouts,
        semantic,
        registration,
        diagnostics,
        foundation,
    )
}
