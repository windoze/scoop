use super::*;

pub(super) struct Context<'a> {
    pub source: &'a hir::CrossConeTypeSemanticsSectionV1,
    pub input: &'a mir::SingleConeStrongMirInput,
    pub authority: mir::MirDispatchSchemaAuthority<'a>,
    pub physical: BTreeMap<PersistentExactTypeId, &'a mir::Type>,
    roots: BTreeMap<mir::FunctionId, CallableOwner>,
}
impl<'a> Context<'a> {
    pub fn new(
        source: &'a hir::CrossConeTypeSemanticsSectionV1,
        input: &'a mir::SingleConeStrongMirInput,
        authority: mir::MirDispatchSchemaAuthority<'a>,
    ) -> Result<Self, Error> {
        let mut physical = BTreeMap::new();
        for record in input.module().meta.source_exact_types.iter() {
            physical.insert(record.identity_record().id(), record.ty());
        }
        let mut roots = BTreeMap::new();
        for root in input.materialization().callable_roots() {
            roots.insert(root.function(), root.implementation());
        }
        Ok(Self {
            source,
            input,
            authority,
            physical,
            roots,
        })
    }
    pub fn target(
        &self,
        function: mir::FunctionId,
    ) -> Result<StrongCallableDefinitionOwner, Error> {
        match *self
            .roots
            .get(&function)
            .ok_or(Error::MissingStrongRoot(function))?
        {
            CallableOwner::Function(id) => Ok(StrongCallableDefinitionOwner::Function(id)),
            CallableOwner::Accessor(id) => Ok(StrongCallableDefinitionOwner::PropertyAccessor(id)),
            CallableOwner::Generated(id) => {
                Ok(StrongCallableDefinitionOwner::GeneratedCallable(id))
            }
            target => Err(Error::InvalidTarget(target)),
        }
    }
    pub fn callable(
        &self,
        target: StrongCallableDefinitionOwner,
    ) -> Result<mir::MirCallableRecordRefV1<'_>, Error> {
        self.authority
            .callables
            .get(target)
            .ok_or(Error::MissingCallable(target))
    }
    pub fn record(
        &self,
        source: &hir::NominalInheritanceInterfaceV1,
        owner: PersistentExactTypeId,
    ) -> Result<mir::ParamFreeMirDispatchSchemaV1, Error> {
        let physical = self
            .physical
            .get(&source.owner())
            .ok_or(Error::MissingPhysicalType(source.owner()))?;
        let mut vtable = mir::MirClassVtableSchemaV1::NoClassVtable;
        let mut itables = reserve(source.slot_schemas().records().len())?;
        for schema in source.slot_schemas().records() {
            let targets = physical::targets(self, source.owner(), physical, schema)?;
            let entries = self.entries(source.owner(), schema, &targets)?;
            match schema.role() {
                hir::InheritanceSlotSchemaRoleV1::ClassVtable => {
                    vtable = mir::MirClassVtableSchemaV1::ClassVtable(entries)
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

pub(super) fn declaration_target(
    declaration: DispatchDeclarationOwner,
) -> StrongCallableDefinitionOwner {
    match declaration {
        DispatchDeclarationOwner::Function(id) => StrongCallableDefinitionOwner::Function(id),
        DispatchDeclarationOwner::Accessor(id) => {
            StrongCallableDefinitionOwner::PropertyAccessor(id)
        }
    }
}
pub(super) fn source_target(
    declaration: hir::InheritanceCallableDeclarationV1,
) -> StrongCallableDefinitionOwner {
    match declaration {
        hir::InheritanceCallableDeclarationV1::Function(id) => {
            StrongCallableDefinitionOwner::Function(id)
        }
        hir::InheritanceCallableDeclarationV1::Getter(id)
        | hir::InheritanceCallableDeclarationV1::Setter(id) => {
            StrongCallableDefinitionOwner::PropertyAccessor(id)
        }
    }
}
