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
    Initialization(Box<MirObjectBridgeError>),
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

    targets: Vec<MirTypeBridgeTargetV1>,
}
impl<'a> Collector<'a> {
    pub fn new(graph: &'a ValidatedIdentityGraph) -> Self {
        Self {
            graph,

            targets: Vec::new(),
        }
    }
    pub fn push(
        &mut self,
        target: MirTypeBridgeTargetV1,
    ) -> Result<(), MirTypeBridgeReferenceError> {
        let path = WirePath::root();

        scoop_wire::allocation::try_reserve(&mut self.targets, 1, &path)?;
        self.targets.push(target);
        Ok(())
    }
    pub fn exact(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<(), MirTypeBridgeReferenceError> {
        self.exact_in(exact, false)
    }
    pub fn field(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<(), MirTypeBridgeReferenceError> {
        self.exact_in(exact, true)
    }
    fn exact_in(
        &mut self,
        exact: PersistentExactTypeId,
        transient: bool,
    ) -> Result<(), MirTypeBridgeReferenceError> {
        let key = self.graph.canonical_key::<_, ExactTypeKey>(exact)?;
        match key.as_ref() {
            ExactTypeKey::Nominal(_) => self.push(MirTypeBridgeTargetV1::Type(exact)),
            ExactTypeKey::Tuple(elements) if transient => {
                for element in elements.as_slice() {
                    self.exact_in(*element, true)?;
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
            self.field(exact)?;
        }
        Ok(())
    }
    pub fn slot(
        &mut self,
        slot: scoop_identity::PersistentDispatchSlotId,
    ) -> Result<(), MirTypeBridgeReferenceError> {
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
        self.targets.sort_unstable();

        self.targets.dedup();
        Ok(MirTypeBridgeSemanticReferencesV1 {
            targets: self.targets,
        })
    }
}
