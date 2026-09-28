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
        for entry in record.vtable().entries() {
            collector.entry(entry, types)?;
        }
        for interface in record.itables() {
            collector.exact(interface.interface())?;
            collector.push(MirTypeBridgeTargetV1::Dispatch(interface.interface()))?;
            for entry in interface.entries() {
                collector.entry(entry, types)?;
            }
        }
        collector.finish()
    }
}
impl Collector<'_> {
    fn entry(
        &mut self,
        entry: &MirDispatchEntryV1,
        types: &dyn MirTypeBridgeTypeLookupV1,
    ) -> Result<(), MirTypeBridgeReferenceError> {
        let key = self
            .graph
            .canonical_key::<_, DispatchSlotKey>(entry.slot())?;
        let receiver = entry
            .signature()
            .exact()
            .receiver()
            .into_option()
            .ok_or(MirTypeBridgeReferenceError::InvalidSlot)?;
        let target = dispatch_declaration_target(self.graph, types, key.owner(), receiver)
            .map_err(|error| MirTypeBridgeReferenceError::Callable(Box::new(error)))?;
        self.push(MirTypeBridgeTargetV1::Callable(target))?;
        self.signature(entry.signature())?;
        self.dispatch_target(entry.implementation().target())
    }
}
