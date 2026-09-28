use super::*;

impl MirTypeBridgeSemanticReferencesV1 {
    pub fn of_type(
        record: &ParamFreeMirTypeExportV1,
        graph: &ValidatedIdentityGraph,
    ) -> Result<Self, MirTypeBridgeReferenceError> {
        let mut collector = Collector::new(graph);
        if let MirTypeRepresentationV1::InlineArray { element } = record.representation() {
            collector.field(*element)?;
        }

        for field in record.representation().fields() {
            collector.field(field.value)?;
        }
        for variant in record.representation().variants() {
            for field in &variant.fields {
                collector.field(field.value)?;
            }
        }
        if let MirBaseClassV1::Base(base) = record.base_and_interfaces().base {
            collector.exact(base)?;
        }
        for interface in &record.base_and_interfaces().interfaces {
            collector.exact(*interface)?;
        }
        if let MirTypeRepresentationV1::Object { backing } = record.representation() {
            collector.exact(*backing)?;
        }
        if let MirTypeOriginV1::GeneratedNominal { role, .. } = record.origin() {
            match role {
                GeneratedNominalKey::ObjectBackingClass { object } => collector.nominal(*object)?,
                GeneratedNominalKey::BoxedValue { payload } => collector.exact(*payload)?,
                GeneratedNominalKey::CoroutineStep { result } => collector.exact(*result)?,
                GeneratedNominalKey::CoroutineSlot { value } => collector.exact(*value)?,
                _ => return Err(MirTypeBridgeReferenceError::GeneratedExecutionGate),
            }
        }
        collector.finish()
    }

    pub fn of_shape(
        record: &ParamFreeMirShapeSupportV1,
        graph: &ValidatedIdentityGraph,
    ) -> Result<Self, MirTypeBridgeReferenceError> {
        let mut collector = Collector::new(graph);
        collector.exact(record.exact())?;
        if let MirBoxedShapeSupportV1::Available(boxed) = record.boxed() {
            collector.exact(boxed)?;
        }
        collector.exact(record.coroutine_step())?;
        collector.exact(record.coroutine_slot())?;
        collector.finish()
    }
}
