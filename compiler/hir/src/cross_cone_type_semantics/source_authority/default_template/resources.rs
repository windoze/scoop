//! Conservative scratch-space bounds before the recursive local-index projection.
use crate::*;
use scoop_identity::{LocalValueSelector, StructuralPathSegment};
use scoop_wire::{BudgetMeter, WireError, WirePath};

pub(super) fn preflight(
    body: &ExportDefaultBodyV1,
    parameters: &CanonicalTemplateValueParametersV1,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let path = WirePath::root();
    let before = meter.usage().decoded_nodes;
    body.visit_definition_sources(&mut |_, _| {}, meter, &path)?;
    let nodes = meter.usage().decoded_nodes - before;
    // The complete walk covers each indexed child, capture, binding and control-flow
    // record. One node reserves all wrapper sizes, including embedded enum payloads;
    // this also bounds vector elements and boxed children without allocating them.
    meter.charge_collection_slots(nodes, &path)?;
    meter.charge_owned_bytes(nodes.saturating_mul(index_node_bound()), &path)?;
    meter.charge_work(nodes, &path)?;
    let count = parameters.parameters().len() as u64;
    meter.charge_collection_slots(count, &path)?;
    meter.charge_owned_bytes(
        count.saturating_mul(std::mem::size_of::<IndexedTemplateValueParameterV1<'_>>() as u64),
        &path,
    )?;
    meter.charge_work(count.saturating_add(1), &path)
}

fn index_node_bound() -> u64 {
    macro_rules! sum_sizes { ($($ty:ty),+ $(,)?) => { 0u64 $(.saturating_add(std::mem::size_of::<$ty>() as u64))+ }; }
    sum_sizes!(
        IndexedExportDefaultBodyV1<'_>,
        IndexedDefaultStatementV1<'_>,
        IndexedDefaultExpressionV1<'_>,
        IndexedDefaultPatternV1<'_>,
        IndexedDefaultPlaceV1,
        IndexedDefaultAssignTargetV1<'_>,
        IndexedDefaultCaptureV1<'_>,
        IndexedDefaultLambdaV1<'_>,
        IndexedDefaultAnonymousFunctionV1<'_>,
        IndexedDefaultLocalFunctionV1<'_>,
        IndexedDefaultCallableReferenceV1<'_>,
        IndexedDefaultBindingShapeV1<'_>,
        IndexedDefaultBindingTemporaryV1<'_>,
        IndexedDefaultBindingLeafV1<'_>,
        IndexedDefaultBindingActionV1<'_>,
        IndexedDefaultBindingPlanV1<'_>,
        IndexedDefaultIteratorConformanceV1<'_>,
        IndexedDefaultAppliedOptionV1<'_>,
        IndexedDefaultIteratorNextV1<'_>,
        IndexedDefaultForIterationPlanV1<'_>,
        IndexedDefaultWhenV1<'_>,
        IndexedDefaultWhenArmV1<'_>,
        IndexedOptionalDefaultWhenGuardV1<'_>,
        IndexedDefaultWhenGuardV1<'_>,
        IndexedDefaultWhenFallbackV1<'_>,
        IndexedDefaultTryV1<'_>,
        IndexedDefaultCatchV1<'_>,
        IndexedOptionalDefaultStatementListV1<'_>,
    )
}

pub(super) struct LocalIndex<'a, 'm> {
    locals: &'a CanonicalTemplateLocalTableV1,
    meter: &'m mut BudgetMeter,
}
impl<'a, 'm> LocalIndex<'a, 'm> {
    pub(super) fn new(
        locals: &'a CanonicalTemplateLocalTableV1,
        meter: &'m mut BudgetMeter,
    ) -> Self {
        Self { locals, meter }
    }
}
impl TemplateLocalIndexResolver for LocalIndex<'_, '_> {
    type Error = DefaultSourceLocalIndexError;
    fn resolve_template_local_index(
        &mut self,
        selector: &LocalValueSelector,
    ) -> Result<u32, Self::Error> {
        use DefaultSourceLocalIndexError as Error;
        let path = WirePath::root();
        let segments = match selector {
            LocalValueSelector::This | LocalValueSelector::Parameter { .. } => 0,
            LocalValueSelector::LocalDeclaration { path }
            | LocalValueSelector::BoundReceiver { path }
            | LocalValueSelector::Synthetic { path, .. }
            | LocalValueSelector::SuspensionResult { site: path } => path.segments().len() as u64,
        };
        self.meter
            .charge_collection_slots(segments, &path)
            .map_err(Error::Resource)?;
        self.meter
            .charge_owned_bytes(
                segments.saturating_mul(std::mem::size_of::<StructuralPathSegment>() as u64),
                &path,
            )
            .map_err(Error::Resource)?;
        let probes = u64::from(self.locals.records().len().max(1).ilog2()) + 1;
        self.meter
            .charge_work(probes.saturating_mul(segments.saturating_add(1)), &path)
            .map_err(Error::Resource)?;
        self.locals
            .index_of(selector)
            .ok_or_else(|| Error::MissingSelector(selector.clone()))
    }
}
