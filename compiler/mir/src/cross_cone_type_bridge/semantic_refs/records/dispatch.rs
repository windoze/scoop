use super::*;

impl MirTypeBridgeSemanticReferencesV1 {
    pub fn of_dispatch(
        record: &ParamFreeMirDispatchSchemaV1,
        graph: &ValidatedIdentityGraph,
        types: &dyn MirTypeBridgeTypeLookupV1,
    ) -> Result<Self, MirTypeBridgeReferenceError> {
        let mut collector = Collector::new(graph);
        collector.exact(record.owner())?;
        let owner = types
            .get(record.owner())
            .ok_or(MirTypeBridgeReferenceError::MissingType(record.owner()))?;
        if let MirBaseClassV1::Base(base) = owner.base_and_interfaces().base {
            collector.push(MirTypeBridgeTargetV1::Dispatch(base))?;
        }
        for interface in &owner.base_and_interfaces().interfaces {
            collector.push(MirTypeBridgeTargetV1::Dispatch(*interface))?;
        }
        for slot in record.interface_slots().into_iter().flatten() {
            collector.signature(slot.signature())?;
        }
        for entry in record.vtable() {
            collector.entry(entry)?;
        }
        for interface in record.itables() {
            collector.exact(interface.interface())?;
            collector.push(MirTypeBridgeTargetV1::Dispatch(interface.interface()))?;
            for entry in interface.entries() {
                collector.entry(entry)?;
            }
        }
        collector.finish()
    }
}
impl Collector<'_> {
    fn entry(&mut self, entry: &MirDispatchEntryV1) -> Result<(), MirTypeBridgeReferenceError> {
        self.signature(entry.signature())?;
        self.dispatch_target(entry.implementation().target())
    }
}
