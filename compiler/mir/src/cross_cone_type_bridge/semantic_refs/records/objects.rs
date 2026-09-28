use super::*;

impl MirTypeBridgeSemanticReferencesV1 {
    pub fn of_object(
        record: &ParamFreeMirObjectValueV1,
        graph: &ValidatedIdentityGraph,
    ) -> Result<Self, MirTypeBridgeReferenceError> {
        let mut collector = Collector::new(graph);
        collector.exact(record.read().object())?;
        collector.exact(record.backing())?;
        collector.push(MirTypeBridgeTargetV1::Callable(
            scoop_identity::CallableDefinitionOwner::Strong(record.ensure()),
        ))?;
        collector.push(MirTypeBridgeTargetV1::InitializationUnit(record.unit()))?;
        collector.finish()
    }

    pub fn of_initialization_use(
        record: &SelectedExternalInitializationUseV1,
        graph: &ValidatedIdentityGraph,
    ) -> Result<Self, MirTypeBridgeReferenceError> {
        let mut collector = Collector::new(graph);
        if super::super::super::objects::unit_provider(graph, record.local_unit())
            .map_err(|error| MirTypeBridgeReferenceError::Initialization(Box::new(error)))?
            .is_some()
        {
            collector.push(MirTypeBridgeTargetV1::InitializationUnit(
                record.local_unit(),
            ))?;
        }
        collector.push(MirTypeBridgeTargetV1::InitializationUnit(
            record.dependency_unit(),
        ))?;
        if let MirExternalInitializationCauseV1::ObjectValue(value) = record.cause() {
            collector.push(MirTypeBridgeTargetV1::Object(value))?;
        }
        // Accessor causes are checked by the existing typed unit relation;
        // their callable can belong to the frozen ordinary bridge partition.
        collector.finish()
    }

    /// Unit ownership is an identity relation, not a type-layout export edge.
    /// Callable roles and bodies are joined with their sealed materialization.
    pub fn of_initialization_unit(
        unit: PersistentInitializationUnitId,
        graph: &ValidatedIdentityGraph,
    ) -> Result<Self, MirTypeBridgeReferenceError> {
        let mut collector = Collector::new(graph);
        collector.unit(unit)?;
        collector.finish()
    }

    pub fn of_initialization_contract(
        unit: PersistentInitializationUnitId,
        signature: &MirBridgeCallableSignatureV1,
        graph: &ValidatedIdentityGraph,
    ) -> Result<Self, MirTypeBridgeReferenceError> {
        let mut collector = Collector::new(graph);
        collector.unit(unit)?;
        collector.signature(signature)?;
        collector.finish()
    }
}
impl Collector<'_> {
    fn unit(
        &mut self,
        unit: PersistentInitializationUnitId,
    ) -> Result<(), MirTypeBridgeReferenceError> {
        super::super::super::objects::unit_provider(self.graph, unit)
            .map_err(|error| MirTypeBridgeReferenceError::Initialization(Box::new(error)))?;
        Ok(())
    }
}
