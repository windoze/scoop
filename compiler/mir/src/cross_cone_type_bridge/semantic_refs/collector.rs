use super::*;

/// Canonical direct dependencies. The enclosing section must resolve every
/// edge against checked local or terminal-provider records before selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirTypeBridgeSemanticReferencesV1 {
    pub(super) targets: Vec<MirTypeBridgeTargetV1>,
}
impl MirTypeBridgeSemanticReferencesV1 {
    pub fn targets(&self) -> &[MirTypeBridgeTargetV1] {
        &self.targets
    }
}

#[derive(Debug)]
pub enum MirTypeBridgeReferenceError {
    Resource(WireError),
    Identity(IdentityReferenceError),
    Hash(scoop_wire::HashError),
    ArithmeticOverflow,
    MissingType(PersistentExactTypeId),
    StructuralExecutionGate(PersistentExactTypeId),
    GenericUnitGate(PersistentInitializationUnitId),
    GeneratedExecutionGate,
    NonMemberCallableTarget(StrongCallableDefinitionOwner),
}
impl From<WireError> for MirTypeBridgeReferenceError {
    fn from(value: WireError) -> Self {
        Self::Resource(value)
    }
}
impl From<IdentityReferenceError> for MirTypeBridgeReferenceError {
    fn from(value: IdentityReferenceError) -> Self {
        Self::Identity(value)
    }
}
impl From<scoop_wire::HashError> for MirTypeBridgeReferenceError {
    fn from(value: scoop_wire::HashError) -> Self {
        Self::Hash(value)
    }
}
impl std::fmt::Display for MirTypeBridgeReferenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "MIR type bridge semantic references: {self:?}")
    }
}
impl std::error::Error for MirTypeBridgeReferenceError {}

pub(super) struct Collector<'a> {
    pub graph: &'a ValidatedIdentityGraph,
    pub meter: &'a mut BudgetMeter,
    targets: Vec<MirTypeBridgeTargetV1>,
}
impl<'a> Collector<'a> {
    pub fn new(graph: &'a ValidatedIdentityGraph, meter: &'a mut BudgetMeter) -> Self {
        Self {
            graph,
            meter,
            targets: Vec::new(),
        }
    }
    pub fn push(
        &mut self,
        target: MirTypeBridgeTargetV1,
    ) -> Result<(), MirTypeBridgeReferenceError> {
        let path = WirePath::root();
        let count = self
            .targets
            .len()
            .checked_add(1)
            .ok_or(MirTypeBridgeReferenceError::ArithmeticOverflow)?;
        self.meter.check_table_entries(count as u64, &path)?;
        self.meter.charge_work(1, &path)?;
        self.meter.charge_edges(1, &path)?;
        self.meter
            .try_reserve_collection_slots(&mut self.targets, 1, &path)?;
        self.targets.push(target);
        Ok(())
    }
    pub fn exact(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<(), MirTypeBridgeReferenceError> {
        self.exact_in(exact, false, 1)
    }
    pub fn field(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<(), MirTypeBridgeReferenceError> {
        self.exact_in(exact, true, 1)
    }
    fn exact_in(
        &mut self,
        exact: PersistentExactTypeId,
        transient: bool,
        depth: u64,
    ) -> Result<(), MirTypeBridgeReferenceError> {
        let path = WirePath::root();
        self.meter.check_semantic_depth(depth, &path)?;
        self.meter.charge_work(1, &path)?;
        self.meter.charge_nodes(1, &path)?;
        let key = self.graph.canonical_key::<_, ExactTypeKey>(exact)?;
        match key.as_ref() {
            ExactTypeKey::Nominal(_) => self.push(MirTypeBridgeTargetV1::Type(exact)),
            ExactTypeKey::Tuple(elements) if transient => {
                let depth = depth
                    .checked_add(1)
                    .ok_or(MirTypeBridgeReferenceError::ArithmeticOverflow)?;
                for element in elements.as_slice() {
                    self.exact_in(*element, true, depth)?;
                }
                Ok(())
            }
            _ => Err(MirTypeBridgeReferenceError::StructuralExecutionGate(exact)),
        }
    }
    pub fn nominal(
        &mut self,
        nominal: PersistentTypeId,
    ) -> Result<(), MirTypeBridgeReferenceError> {
        let key = ExactTypeKey::Nominal(nominal);
        self.meter.charge_sha256(
            PersistentExactTypeId::hash_stream_length(&key)?,
            &WirePath::root(),
        )?;
        self.exact(PersistentExactTypeId::from_key(&key)?)
    }
    pub fn signature(
        &mut self,
        signature: &MirBridgeCallableSignatureV1,
    ) -> Result<(), MirTypeBridgeReferenceError> {
        for exact in signature
            .exact()
            .receiver()
            .into_option()
            .into_iter()
            .chain(signature.exact().parameters().iter().copied())
            .chain([signature.exact().result()])
        {
            self.exact(exact)?;
        }
        Ok(())
    }
    pub fn slot(
        &mut self,
        slot: scoop_identity::PersistentDispatchSlotId,
    ) -> Result<(), MirTypeBridgeReferenceError> {
        self.meter.charge_work(1, &WirePath::root())?;
        let key = self.graph.canonical_key::<_, DispatchSlotKey>(slot)?;
        self.member_target(match key.owner() {
            DispatchDeclarationOwner::Function(id) => StrongCallableDefinitionOwner::Function(id),
            DispatchDeclarationOwner::Accessor(id) => {
                StrongCallableDefinitionOwner::PropertyAccessor(id)
            }
        })
    }
    pub fn finish(
        mut self,
    ) -> Result<MirTypeBridgeSemanticReferencesV1, MirTypeBridgeReferenceError> {
        let count = self.targets.len();
        let path = WirePath::root();
        for _ in 0..usize::BITS - count.max(1).saturating_sub(1).leading_zeros() {
            self.meter.charge_work(count as u64, &path)?;
        }
        self.targets.sort_unstable();
        self.meter.charge_work(count as u64, &path)?;
        self.targets.dedup();
        Ok(MirTypeBridgeSemanticReferencesV1 {
            targets: self.targets,
        })
    }
}
