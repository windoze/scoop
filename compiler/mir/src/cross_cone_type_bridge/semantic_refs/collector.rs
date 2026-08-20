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
    InvalidSlot,
    Callable(Box<MirCallableBridgeError>),
    DefinitionReference(scoop_identity::CallableBodyResolutionError<IdentityReferenceError>),
    Resource(WireError),
    Identity(IdentityReferenceError),
    Hash(scoop_wire::HashError),
    ArithmeticOverflow,
    MissingType(PersistentExactTypeId),
    Initialization(Box<MirObjectBridgeError>),
    GeneratedExecutionGate,
    NonMemberCallableTarget(CallableDefinitionOwner),
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
        let key = self.graph.canonical_key::<_, ExactTypeKey>(exact)?;
        match key.as_ref() {
            ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. } => {
                self.push(MirTypeBridgeTargetV1::Type(exact))
            }
            ExactTypeKey::Tuple(elements) => {
                for element in elements.as_slice() {
                    self.exact(*element)?;
                }
                Ok(())
            }
            // Structural pointer values use their target representation. Their
            // pointee/signature identities do not require a physical type export.
            ExactTypeKey::Function { .. }
            | ExactTypeKey::RawPointer(_)
            | ExactTypeKey::NativeFunctionPointer { .. } => Ok(()),
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
            self.exact(exact)?;
        }
        Ok(())
    }
    pub fn slot(
        &mut self,
        slot: scoop_identity::PersistentDispatchSlotId,
    ) -> Result<(), MirTypeBridgeReferenceError> {
        self.graph.canonical_key::<_, DispatchSlotKey>(slot)?;
        Ok(())
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

impl From<scoop_identity::CallableBodyResolutionError<IdentityReferenceError>>
    for MirTypeBridgeReferenceError
{
    fn from(error: scoop_identity::CallableBodyResolutionError<IdentityReferenceError>) -> Self {
        Self::DefinitionReference(error)
    }
}
