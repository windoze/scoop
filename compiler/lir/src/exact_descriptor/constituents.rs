use super::*;
use crate::{
    CanonicalExactDispatchExportsV1, CanonicalExactLayoutExportsV1, ConeLirFoundation,
    ExternalStrongShapeSubjectV1, LirTargetProfile, StrongShapeDefinitionRefV1,
    StrongShapeRegistrationV1, StrongTypeDescriptorRefV2, StrongTypeDescriptorSemanticPlanV2,
    TypeDescriptorInlineScanV1,
};
use scoop_identity::{
    CanonicalExactTypeDiagnosticName, ExactTypeDiagnosticGraph, PersistentExactTypeId,
    RepresentationRole,
};
use scoop_wire::WirePath;

mod tables;

/// Source-derived ancestry. Layout, dispatch and definition claims are
/// supplied separately as already checked constituents.
#[derive(Clone, Copy)]
pub struct ExactDescriptorSourceInputV1<'a> {
    pub exact: PersistentExactTypeId,
    pub release_policy: crate::ReleasePolicy,
    pub parent: Option<StrongTypeDescriptorRefV2>,
    pub interfaces: &'a [StrongTypeDescriptorRefV2],
    pub interface_parents: Option<&'a [StrongTypeDescriptorRefV2]>,
}

impl ExactDescriptorExportV1 {
    /// Reconstructs all semantic and registration fields without accepting
    /// a caller-provided expected descriptor or physical registration plan.
    pub fn replay_from_constituents(
        target: LirTargetProfile,
        source: ExactDescriptorSourceInputV1<'_>,
        layouts: &CanonicalExactLayoutExportsV1,
        dispatch: &CanonicalExactDispatchExportsV1,
        diagnostics: &impl ExactTypeDiagnosticGraph,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, ExactDescriptorError> {
        if layouts.provider() != foundation.producer()
            || dispatch.provider() != foundation.producer()
        {
            return Err(ExactDescriptorError::Provider);
        }
        if layouts.target() != target || dispatch.target() != target {
            return Err(ExactDescriptorError::Target);
        }
        let exact = source.exact;

        let instance = layouts
            .find_exact_role(exact, RepresentationRole::ManagedObject)
            .ok_or(ExactDescriptorError::MissingInstanceLayout(exact))?;
        let layout = instance
            .instance_handle()
            .ok_or(ExactDescriptorError::LayoutRole(
                instance.identity().layout(),
            ))?;
        let inline_scan = match replay::expected_inline_scan(instance, &layout, layouts)? {
            Some(scan) => TypeDescriptorInlineScanV1::Defined(scan),
            None => TypeDescriptorInlineScanV1::Null,
        };
        let name = CanonicalExactTypeDiagnosticName::from_validated_graph(exact, diagnostics)?;
        let (vtable, itables) = tables::replay(source, dispatch)?;

        let semantic = StrongTypeDescriptorSemanticPlanV2::from_artifact(
            exact,
            name.as_str().to_owned(),
            instance.identity().layout(),
            instance.scan(),
            layout.shape().clone(),
            inline_scan,
            source.parent,
            vtable,
            itables,
            source
                .interface_parents
                .map_or(crate::TypeDescriptorRelations::Absent, |parents| {
                    crate::TypeDescriptorRelations::Interface {
                        parents: parents.iter().copied().map(Some).collect(),
                    }
                }),
            source.release_policy,
        );
        let physical = StrongShapeDefinitionRefV1::from_foundation(
            ExternalStrongShapeSubjectV1::TypeRegistration(exact),
            foundation,
        )?;
        let fingerprint =
            replay::registration_fingerprint(exact, physical.definition(), foundation)?;
        let registration = StrongShapeRegistrationV1::from_artifact(
            exact,
            physical.definition(),
            physical.symbol(),
            fingerprint,
        );
        replay::replay_parts(
            target,
            layouts,
            &semantic,
            registration,
            diagnostics,
            foundation,
        )
    }
}
