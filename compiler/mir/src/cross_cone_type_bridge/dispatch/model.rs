use super::*;

/// An ordinal meaningful only inside one exact owner's dispatch table.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MirDispatchPositionV1(u32);
impl MirDispatchPositionV1 {
    pub const fn new(position: u32) -> Self {
        Self(position)
    }
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirDispatchReceiverAdaptationV1 {
    Identity,
    ReferenceDispatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirDispatchImplementationV1 {
    AbstractObligation {
        declaration: DispatchDeclarationOwner,
        trap_target: CallableDefinitionOwner,
        receiver: MirDispatchReceiverAdaptationV1,
    },
    DirectStrongTarget {
        target: CallableDefinitionOwner,
        receiver: MirDispatchReceiverAdaptationV1,
    },
    InterfaceDefaultTarget {
        target: CallableDefinitionOwner,
        receiver: MirDispatchReceiverAdaptationV1,
    },
    AdjustThunkTarget(CallableDefinitionOwner),
}
impl MirDispatchImplementationV1 {
    pub const fn target(self) -> CallableDefinitionOwner {
        match self {
            Self::AbstractObligation { trap_target, .. } => trap_target,
            Self::DirectStrongTarget { target, .. }
            | Self::InterfaceDefaultTarget { target, .. }
            | Self::AdjustThunkTarget(target) => target,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirDispatchSlotV1 {
    pub(super) slot: PersistentDispatchSlotId,
    pub(super) position: MirDispatchPositionV1,
    pub(super) signature: MirBridgeCallableSignatureV1,
}
impl MirDispatchSlotV1 {
    pub fn new(
        slot: PersistentDispatchSlotId,
        position: MirDispatchPositionV1,
        signature: MirBridgeCallableSignatureV1,
    ) -> Self {
        Self {
            slot,
            position,
            signature,
        }
    }
    pub const fn slot(&self) -> PersistentDispatchSlotId {
        self.slot
    }
    pub const fn position(&self) -> MirDispatchPositionV1 {
        self.position
    }
    pub const fn signature(&self) -> &MirBridgeCallableSignatureV1 {
        &self.signature
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirDispatchEntryV1 {
    pub(super) contract: MirDispatchSlotV1,
    pub(super) implementation: MirDispatchImplementationV1,
}
impl MirDispatchEntryV1 {
    pub fn new(
        slot: PersistentDispatchSlotId,
        position: MirDispatchPositionV1,
        signature: MirBridgeCallableSignatureV1,
        implementation: MirDispatchImplementationV1,
    ) -> Self {
        Self::from_contract(
            MirDispatchSlotV1::new(slot, position, signature),
            implementation,
        )
    }
    pub fn from_contract(
        contract: MirDispatchSlotV1,
        implementation: MirDispatchImplementationV1,
    ) -> Self {
        Self {
            contract,
            implementation,
        }
    }
    pub const fn contract(&self) -> &MirDispatchSlotV1 {
        &self.contract
    }
    pub const fn slot(&self) -> PersistentDispatchSlotId {
        self.contract.slot()
    }
    pub const fn position(&self) -> MirDispatchPositionV1 {
        self.contract.position()
    }
    pub const fn signature(&self) -> &MirBridgeCallableSignatureV1 {
        self.contract.signature()
    }
    pub const fn implementation(&self) -> MirDispatchImplementationV1 {
        self.implementation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirDispatchSlotsV1 {
    NoClassVtable,
    ClassVtable(Vec<MirDispatchEntryV1>),
    InterfaceSlots(Vec<MirDispatchSlotV1>),
}
impl MirDispatchSlotsV1 {
    fn vtable(&self) -> &[MirDispatchEntryV1] {
        match self {
            Self::NoClassVtable | Self::InterfaceSlots(_) => &[],
            Self::ClassVtable(entries) => entries,
        }
    }
    fn interface_slots(&self) -> Option<&[MirDispatchSlotV1]> {
        match self {
            Self::InterfaceSlots(slots) => Some(slots),
            Self::NoClassVtable | Self::ClassVtable(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirInterfaceDispatchTableV1 {
    pub(super) interface: PersistentExactTypeId,
    pub(super) entries: Vec<MirDispatchEntryV1>,
}
impl MirInterfaceDispatchTableV1 {
    pub fn new(interface: PersistentExactTypeId, entries: Vec<MirDispatchEntryV1>) -> Self {
        Self { interface, entries }
    }
    pub const fn interface(&self) -> PersistentExactTypeId {
        self.interface
    }
    pub fn entries(&self) -> &[MirDispatchEntryV1] {
        &self.entries
    }
}

/// One checked owner record. Cross-record prefix/provider closure is proved
/// by the canonical schema table rather than inferred from sorted slot ids.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamFreeMirDispatchSchemaV1 {
    odr: bool,
    pub(super) owner: PersistentExactTypeId,
    pub(super) slots: MirDispatchSlotsV1,
    pub(super) itables: Vec<MirInterfaceDispatchTableV1>,
}
impl ParamFreeMirDispatchSchemaV1 {
    pub fn try_new(
        authority: MirDispatchSchemaAuthority<'_>,
        owner: PersistentExactTypeId,
        slots: MirDispatchSlotsV1,
        itables: Vec<MirInterfaceDispatchTableV1>,
    ) -> Result<Self, MirDispatchSchemaError> {
        let record = Self {
            odr: authority.type_export(owner)?.is_odr(),
            owner,
            slots,
            itables,
        };
        authority.validate_record(&record)?;
        Ok(record)
    }
    pub const fn owner(&self) -> PersistentExactTypeId {
        self.owner
    }
    pub(in crate::cross_cone_type_bridge) const fn is_odr(&self) -> bool {
        self.odr
    }
    pub const fn slots(&self) -> &MirDispatchSlotsV1 {
        &self.slots
    }
    pub fn vtable(&self) -> &[MirDispatchEntryV1] {
        self.slots.vtable()
    }
    pub fn interface_slots(&self) -> Option<&[MirDispatchSlotV1]> {
        self.slots.interface_slots()
    }
    pub fn itables(&self) -> &[MirInterfaceDispatchTableV1] {
        &self.itables
    }
    pub fn interface_table(
        &self,
        interface: PersistentExactTypeId,
    ) -> Option<&MirInterfaceDispatchTableV1> {
        self.itables
            .binary_search_by_key(&interface, MirInterfaceDispatchTableV1::interface)
            .ok()
            .map(|index| &self.itables[index])
    }
}

#[derive(Clone, Copy)]
pub struct MirDispatchSchemaAuthority<'a> {
    pub identities: &'a ValidatedIdentityGraph,
    pub types: &'a dyn MirTypeBridgeTypeLookupV1,
    pub callables: &'a dyn MirTypeBridgeCallableLookupV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMirDispatchSchemasV1 {
    pub(super) records: Vec<ParamFreeMirDispatchSchemaV1>,
}
impl CanonicalMirDispatchSchemasV1 {
    pub fn try_new(
        authority: MirDispatchSchemaAuthority<'_>,
        records: Vec<ParamFreeMirDispatchSchemaV1>,
    ) -> Result<Self, MirDispatchSchemaError> {
        Self::try_new_with_dependencies(authority, records, &[])
    }
    /// Dependency schemas remain owned by their defining tables.
    pub fn try_new_with_dependencies(
        authority: MirDispatchSchemaAuthority<'_>,
        mut records: Vec<ParamFreeMirDispatchSchemaV1>,
        dependencies: &[&CanonicalMirDispatchSchemasV1],
    ) -> Result<Self, MirDispatchSchemaError> {
        records.sort_unstable_by_key(ParamFreeMirDispatchSchemaV1::owner);
        if let Some(pair) = records
            .windows(2)
            .find(|pair| pair[0].owner == pair[1].owner)
        {
            return Err(MirDispatchSchemaError::DuplicateOwner {
                owner: pair[0].owner,
            });
        }
        let table = Self { records };
        authority.validate_with_dependencies(&table, dependencies)?;
        Ok(table)
    }
    pub fn records(&self) -> &[ParamFreeMirDispatchSchemaV1] {
        &self.records
    }
    pub fn get(&self, owner: PersistentExactTypeId) -> Option<&ParamFreeMirDispatchSchemaV1> {
        self.records
            .binary_search_by_key(&owner, ParamFreeMirDispatchSchemaV1::owner)
            .ok()
            .map(|index| &self.records[index])
    }
}
