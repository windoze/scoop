use super::*;

pub(super) struct Context<'a> {
    pub source: &'a hir::CrossConeTypeSemanticsSectionV1,
    pub input: &'a mir::ConeMirInput,
    pub authority: mir::MirDispatchSchemaAuthority<'a>,
    pub physical: BTreeMap<PersistentExactTypeId, &'a mir::Type>,
    roots: BTreeMap<mir::FunctionId, CallableDefinitionOwner>,
}
impl<'a> Context<'a> {
    pub fn new(
        source: &'a hir::CrossConeTypeSemanticsSectionV1,
        input: &'a mir::ConeMirInput,
        authority: mir::MirDispatchSchemaAuthority<'a>,
    ) -> Result<Self, Error> {
        let mut physical = BTreeMap::new();
        for record in input.module().meta.source_exact_types.iter() {
            physical.insert(record.identity_record().id(), record.ty());
        }
        let mut roots = BTreeMap::new();
        for root in input.materialization().callable_roots() {
            let target =
                CallableDefinitionOwner::try_from(root.subject()).map_err(Error::InvalidTarget)?;
            roots.insert(root.function(), target);
        }
        Ok(Self {
            source,
            input,
            authority,
            physical,
            roots,
        })
    }
    pub fn target(&self, function: mir::FunctionId) -> Result<CallableDefinitionOwner, Error> {
        self.roots
            .get(&function)
            .copied()
            .ok_or(Error::MissingStrongRoot(function))
    }

    pub fn callable(
        &self,
        target: CallableDefinitionOwner,
    ) -> Result<mir::MirCallableRecordRefV1<'_>, Error> {
        self.authority
            .callables
            .get(target)
            .ok_or(Error::MissingCallable(target))
    }
    pub fn record(
        &self,
        local: &hir::LocalConcreteHir,
        source: &hir::NominalInheritanceInterfaceV1,
        owner: PersistentExactTypeId,
    ) -> Result<mir::ParamFreeMirDispatchSchemaV1, Error> {
        let physical = self
            .physical
            .get(&source.owner())
            .ok_or(Error::MissingPhysicalType(source.owner()))?;
        if matches!(physical, mir::Type::Interface(_)) {
            return mir::ParamFreeMirDispatchSchemaV1::try_new(
                self.authority,
                owner,
                mir::MirDispatchSlotsV1::InterfaceSlots(slots::interface(local, owner)?),
                vec![],
            )
            .map_err(Error::Schema);
        }
        let mut vtable = mir::MirDispatchSlotsV1::NoClassVtable;
        let mut itables = reserve(source.slot_schemas().records().len())?;
        for schema in source.slot_schemas().records() {
            let targets = physical::targets(self, source.owner(), physical, schema)?;
            let entries = self.entries(source.owner(), schema, &targets)?;
            match schema.role() {
                hir::InheritanceSlotSchemaRoleV1::ClassVtable => {
                    vtable = mir::MirDispatchSlotsV1::ClassVtable(entries)
                }
                hir::InheritanceSlotSchemaRoleV1::Interface { interface_exact } => itables.push(
                    mir::MirInterfaceDispatchTableV1::new(interface_exact, entries),
                ),
            }
        }
        mir::ParamFreeMirDispatchSchemaV1::try_new(self.authority, owner, vtable, itables)
            .map_err(Error::Schema)
    }
}
