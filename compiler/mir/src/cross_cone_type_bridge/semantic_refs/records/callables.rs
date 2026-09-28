use super::*;

impl MirTypeBridgeSemanticReferencesV1 {
    pub fn of_callable(
        record: MirCallableRecordRefV1<'_>,
        graph: &ValidatedIdentityGraph,
    ) -> Result<Self, MirTypeBridgeReferenceError> {
        let mut collector = Collector::new(graph);
        collector.signature(record.semantic_signature())?;
        if record.lowered_signature() != record.semantic_signature() {
            collector.signature(record.lowered_signature())?;
        }
        match *record.lowering_role() {
            MirCallableLoweringRoleV1::ClassInitializer { owner }
            | MirCallableLoweringRoleV1::ValueConstructor { owner }
            | MirCallableLoweringRoleV1::PrimaryValueConstructor { owner }
            | MirCallableLoweringRoleV1::DerivedEquality { owner } => collector.exact(owner)?,
            MirCallableLoweringRoleV1::DispatchAdjust { target }
            | MirCallableLoweringRoleV1::BoxingAdjust { target } => {
                collector.member_target(target)?;
            }
            MirCallableLoweringRoleV1::ObjectEnsure { unit }
            | MirCallableLoweringRoleV1::ObjectInitializer { unit } => {
                collector.push(MirTypeBridgeTargetV1::InitializationUnit(unit))?;
            }
            MirCallableLoweringRoleV1::PureVirtualTrap { slot } => collector.slot(slot)?,
            MirCallableLoweringRoleV1::Ordinary | MirCallableLoweringRoleV1::Accessor => {}
        }
        if let MirCallableOriginV1::Generated { role, .. } = record.origin().as_ref() {
            match role {
                GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor } => {
                    collector.push(MirTypeBridgeTargetV1::Callable(
                        scoop_identity::CallableDefinitionOwner::Strong(
                            StrongCallableDefinitionOwner::Constructor(*constructor),
                        ),
                    ))?;
                }
                GeneratedCallableKey::Initialization { unit, .. } => {
                    collector.push(MirTypeBridgeTargetV1::InitializationUnit(*unit))?;
                }
                GeneratedCallableKey::DerivedEquality { exact_owner } => {
                    collector.exact(*exact_owner)?
                }
                GeneratedCallableKey::DispatchAdjust {
                    slot, implementor, ..
                } => {
                    collector.slot(*slot)?;
                    collector.exact(*implementor)?;
                }
                GeneratedCallableKey::BoxingAdjust {
                    slot,
                    payload,
                    interface,
                } => {
                    collector.slot(*slot)?;
                    collector.exact(*payload)?;
                    collector.exact(*interface)?;
                }
                _ => return Err(MirTypeBridgeReferenceError::GeneratedExecutionGate),
            }
        }
        collector.finish()
    }
}
