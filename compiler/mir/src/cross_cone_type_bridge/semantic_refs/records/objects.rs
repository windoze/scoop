use super::*;

impl MirTypeBridgeSemanticReferencesV1 {
    pub fn of_object(
        record: &ParamFreeMirObjectValueV1,
        graph: &ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<Self, MirTypeBridgeReferenceError> {
        let mut collector = Collector::new(graph, meter);
        collector.exact(record.read().object())?;
        collector.exact(record.backing())?;
        collector.push(MirTypeBridgeTargetV1::Callable(record.ensure()))?;
        collector.push(MirTypeBridgeTargetV1::InitializationUnit(record.unit()))?;
        collector.finish()
    }

    pub fn of_initialization_use(
        record: &SelectedExternalInitializationUseV1,
        graph: &ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<Self, MirTypeBridgeReferenceError> {
        let mut collector = Collector::new(graph, meter);
        collector.push(MirTypeBridgeTargetV1::InitializationUnit(
            record.local_unit(),
        ))?;
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

    /// The unit's callable roles and bodies are separately joined with the
    /// sealed materialization roots; deriving its source edges is not proof.
    pub fn of_initialization_unit(
        unit: PersistentInitializationUnitId,
        graph: &ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<Self, MirTypeBridgeReferenceError> {
        let mut collector = Collector::new(graph, meter);
        collector.unit(unit)?;
        collector.finish()
    }

    pub fn of_initialization_contract(
        unit: PersistentInitializationUnitId,
        signature: &MirBridgeCallableSignatureV1,
        graph: &ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<Self, MirTypeBridgeReferenceError> {
        let mut collector = Collector::new(graph, meter);
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
        self.meter.charge_work(1, &WirePath::root())?;
        let key = self.graph.canonical_key::<_, InitializationUnitKey>(unit)?;
        match key.as_ref() {
            InitializationUnitKey::Object(object) | InitializationUnitKey::Companion(object) => {
                self.nominal(*object)?;
            }
            InitializationUnitKey::TopLevelProperty(_)
            | InitializationUnitKey::ExtensionProperty(_) => {}
            InitializationUnitKey::GenericDelegatedExtensionApplication { .. } => {
                return Err(MirTypeBridgeReferenceError::GenericUnitGate(unit));
            }
        }
        Ok(())
    }
}
