use std::collections::BTreeMap;

use scoop_identity::{
    DispatchDeclarationOwner, DispatchRole, ExactTypeKey, PersistentDispatchSlotId,
    PersistentTypeId,
};
use scoop_wire::{BudgetMeter, WirePath};

use super::source_resources::{invalid, resource, work};
use super::*;

type Declaration = InheritanceCallableDeclarationV1;

/// Joins resolved source members and implementation choices without changing
/// slot identity, root visibility, or the schema's declaration order.
pub(in crate::production::type_semantics) struct SlotContracts<'a> {
    export: &'a ExportHir,
    sources: &'a CanonicalInheritanceSourceCallablesV1,
    selections: &'a CanonicalInheritanceSourceSlotSelectionsV1,
    functions: BTreeMap<Declaration, FunctionId>,
    roots: BTreeMap<PersistentDispatchSlotId, Declaration>,
}

impl<'a> SlotContracts<'a> {
    pub(in crate::production::type_semantics) fn new(
        export: &'a ExportHir,
        sources: &'a CanonicalInheritanceSourceCallablesV1,
        selections: &'a CanonicalInheritanceSourceSlotSelectionsV1,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let mut functions = BTreeMap::new();
        for (id, _) in export.functions.iter() {
            work(meter, sources.records().len())?;
            if let Some(declaration) = source_callables::identity(export, id)
                && sources.get(declaration).is_some()
            {
                insert(&mut functions, declaration, id, meter)?;
            }
        }
        let mut roots = BTreeMap::new();
        for record in export.dispatch_slot_identities.records() {
            work(meter, sources.records().len())?;
            let declaration = match (record.key().owner(), record.key().role()) {
                (
                    DispatchDeclarationOwner::Function(id),
                    DispatchRole::VirtualMethod | DispatchRole::InterfaceMethod,
                ) => Declaration::Function(id),
                (DispatchDeclarationOwner::Accessor(id), DispatchRole::PropertyGetter) => {
                    Declaration::Getter(id)
                }
                (DispatchDeclarationOwner::Accessor(id), DispatchRole::PropertySetter) => {
                    Declaration::Setter(id)
                }
                _ => {
                    return Err(invalid(
                        "dispatch role disagrees with its sealed declaration identity",
                    ));
                }
            };
            if sources.get(declaration).is_some() {
                insert(&mut roots, record.id(), declaration, meter)?;
            }
        }
        Ok(Self {
            export,
            sources,
            selections,
            functions,
            roots,
        })
    }

    pub(super) fn project(
        &self,
        owner: PersistentExactTypeId,
        schemas: &CanonicalInheritanceSlotSchemasV1,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalInheritanceSlotContractsV1, Error> {
        let mut contracts = BTreeMap::new();
        for schema in schemas.records() {
            for slot in schema.slots() {
                work(meter, contracts.len())?;
                if contracts.contains_key(slot) {
                    continue;
                }
                work(meter, self.roots.len())?;
                let declaration = *self
                    .roots
                    .get(slot)
                    .ok_or_else(|| invalid("dispatch slot has no source root declaration"))?;
                let (source, function, declaration_owner) = self.source(declaration, meter)?;
                work(meter, self.selections.records().len())?;
                let selection = self.selections.get(owner, *slot).ok_or_else(|| {
                    invalid("dispatch slot has no resolved implementation selection")
                })?;
                let implementation = match selection {
                    InheritanceSourceSlotSelectionV1::Abstract => {
                        InheritanceSlotImplementationV1::Abstract
                    }
                    InheritanceSourceSlotSelectionV1::Concrete(target) => {
                        InheritanceSlotImplementationV1::Concrete(self.target(target, meter)?)
                    }
                    InheritanceSourceSlotSelectionV1::InterfaceDefault(target) => {
                        InheritanceSlotImplementationV1::InterfaceDefault(
                            self.target(target, meter)?,
                        )
                    }
                };
                let contract = InheritanceSlotContractV1::try_new(
                    *slot,
                    declaration_owner,
                    declaration,
                    source.signature().clone(),
                    PersistentSlotContractDomainV1::new(domain(
                        self.export,
                        &function.access.lookup.0,
                    )?),
                    implementation,
                    source.declaration_access().clone(),
                )
                .map_err(invalid)?;
                insert(&mut contracts, *slot, contract, meter)?;
            }
        }
        meter
            .charge_collection_slots(contracts.len() as u64, &WirePath::root())
            .map_err(resource)?;
        CanonicalInheritanceSlotContractsV1::try_new(contracts.into_values().collect())
            .map_err(invalid)
    }

    fn source(
        &self,
        declaration: Declaration,
        meter: &mut BudgetMeter,
    ) -> Result<(&InheritanceSourceCallableV1, &Function, PersistentTypeId), Error> {
        work(meter, self.sources.records().len())?;
        let source = self
            .sources
            .get(declaration)
            .ok_or_else(|| invalid("dispatch declaration has no source callable contract"))?;
        work(meter, self.functions.len())?;
        let function = &self.export.functions[*self
            .functions
            .get(&declaration)
            .ok_or_else(|| invalid("dispatch declaration has no source function"))?];
        let method = function
            .method
            .ok_or_else(|| invalid("dispatch declaration has no member owner"))?;
        let identity = self.export.type_identities[method.owner]
            .exact()
            .ok_or(Error::MissingExactIdentity)?;
        let ExactTypeKey::Nominal(owner) = identity.key() else {
            return Err(Error::GenericOdrRequired(identity.id()));
        };
        Ok((source, function, *owner))
    }

    fn target(
        &self,
        declaration: Declaration,
        meter: &mut BudgetMeter,
    ) -> Result<InheritanceSlotTargetV1, Error> {
        let (source, _, owner) = self.source(declaration, meter)?;
        InheritanceSlotTargetV1::try_new(
            declaration,
            owner,
            source.signature().clone(),
            source.modality(),
            source.declaration_access().clone(),
        )
        .map_err(invalid)
    }
}

fn insert<K: Ord, V>(
    map: &mut BTreeMap<K, V>,
    key: K,
    value: V,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    work(meter, map.len())?;
    meter
        .check_table_entries(map.len() as u64 + 1, &WirePath::root())
        .map_err(resource)?;
    meter
        .charge_collection_slots(1, &WirePath::root())
        .map_err(resource)?;
    if map.insert(key, value).is_some() {
        return Err(invalid("duplicate dispatch projection identity"));
    }
    Ok(())
}
