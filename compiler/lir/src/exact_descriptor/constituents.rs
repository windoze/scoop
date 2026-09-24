use super::*;
use crate::{
    CanonicalExactDispatchExportsV1, CanonicalExactLayoutExportsV1, ExternalStrongShapeSubjectV1,
    LirTargetProfile, OdrFreeLirFoundation, StrongShapeDefinitionRefV1, StrongShapeRegistrationV1,
    StrongTypeDescriptorRefV2, StrongTypeDescriptorSemanticPlanV2, TypeDescriptorInlineScanV1,
};
use scoop_identity::{
    CanonicalExactTypeDiagnosticName, ExactTypeDiagnosticGraph, PersistentExactTypeId,
    RepresentationRole,
};
use scoop_wire::{BudgetMeter, WirePath};

mod tables;

/// Source-derived ancestry. Layout, dispatch and definition claims are
/// supplied separately as already checked constituents.
#[derive(Clone, Copy)]
pub struct ExactDescriptorSourceInputV1<'a> {
    pub exact: PersistentExactTypeId,
    pub parent: Option<StrongTypeDescriptorRefV2>,
    pub interfaces: &'a [StrongTypeDescriptorRefV2],
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
        foundation: &OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
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
        let path = WirePath::root();
        meter.charge_work(layouts.records().len() as u64, &path)?;
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
        let name = CanonicalExactTypeDiagnosticName::from_validated_graph_metered(
            exact,
            diagnostics,
            meter,
        )?;
        let (vtable, itables) = tables::replay(source, dispatch, meter)?;
        resources::shape(layout.shape(), meter)?;
        meter.charge_owned_bytes(name.as_str().len() as u64, &path)?;
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
        );
        let physical = StrongShapeDefinitionRefV1::from_foundation(
            ExternalStrongShapeSubjectV1::TypeRegistration(exact),
            foundation,
            meter,
        )?;
        let fingerprint = scoop_identity::DigestNodeId::from_key(
            &scoop_identity::DigestNodeKey::strong_registration(physical.definition()),
        )?;
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
            meter,
        )
    }
}
