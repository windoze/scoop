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
        trap_target: StrongCallableDefinitionOwner,
        receiver: MirDispatchReceiverAdaptationV1,
    },
    DirectStrongTarget {
        target: StrongCallableDefinitionOwner,
        receiver: MirDispatchReceiverAdaptationV1,
    },
    InterfaceDefaultTarget {
        target: StrongCallableDefinitionOwner,
        receiver: MirDispatchReceiverAdaptationV1,
    },
    AdjustThunkTarget(StrongCallableDefinitionOwner),
}
impl MirDispatchImplementationV1 {
    pub const fn target(self) -> StrongCallableDefinitionOwner {
        match self {
            Self::AbstractObligation { trap_target, .. } => trap_target,
            Self::DirectStrongTarget { target, .. }
            | Self::InterfaceDefaultTarget { target, .. }
            | Self::AdjustThunkTarget(target) => target,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirDispatchEntryV1 {
    pub(super) slot: PersistentDispatchSlotId,
    pub(super) position: MirDispatchPositionV1,
    pub(super) signature: MirBridgeCallableSignatureV1,
    pub(super) implementation: MirDispatchImplementationV1,
}
impl MirDispatchEntryV1 {
    pub fn new(
        slot: PersistentDispatchSlotId,
        position: MirDispatchPositionV1,
        signature: MirBridgeCallableSignatureV1,
        implementation: MirDispatchImplementationV1,
    ) -> Self {
        Self {
            slot,
            position,
            signature,
            implementation,
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
    pub const fn implementation(&self) -> MirDispatchImplementationV1 {
        self.implementation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirClassVtableSchemaV1 {
    NoClassVtable,
    ClassVtable(Vec<MirDispatchEntryV1>),
}
impl MirClassVtableSchemaV1 {
    pub fn entries(&self) -> &[MirDispatchEntryV1] {
        match self {
            Self::NoClassVtable => &[],
            Self::ClassVtable(entries) => entries,
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
    pub(super) owner: PersistentExactTypeId,
    pub(super) vtable: MirClassVtableSchemaV1,
    pub(super) itables: Vec<MirInterfaceDispatchTableV1>,
}
impl ParamFreeMirDispatchSchemaV1 {
    pub fn try_new(
        authority: MirDispatchSchemaAuthority<'_>,
        owner: PersistentExactTypeId,
        vtable: MirClassVtableSchemaV1,
        itables: Vec<MirInterfaceDispatchTableV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, MirDispatchSchemaError> {
        let record = Self {
            owner,
            vtable,
            itables,
        };
        authority.validate_record(&record, meter)?;
        Ok(record)
    }
    pub const fn owner(&self) -> PersistentExactTypeId {
        self.owner
    }
    pub const fn vtable(&self) -> &MirClassVtableSchemaV1 {
        &self.vtable
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
    pub types: &'a CanonicalParamFreeMirTypeExportsV1,
    pub callables: &'a CanonicalMirCallableBindingsV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMirDispatchSchemasV1 {
    pub(super) records: Vec<ParamFreeMirDispatchSchemaV1>,
}
impl CanonicalMirDispatchSchemasV1 {
    pub fn try_new(
        authority: MirDispatchSchemaAuthority<'_>,
        mut records: Vec<ParamFreeMirDispatchSchemaV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, MirDispatchSchemaError> {
        meter.check_table_entries(records.len() as u64, &WirePath::root())?;
        charge_sort(records.len(), meter)?;
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
        authority.validate_table(&table, meter)?;
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
