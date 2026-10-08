use std::collections::BTreeMap;

use scoop_identity::{
    DispatchDeclarationOwner, DispatchRole, DispatchSlotKey, PersistentDispatchSlotId,
};

use super::source_errors::invalid;
use super::*;

type Declaration = InheritanceCallableDeclarationV1;

mod equality;

/// Projects the checked declaration with each actual receiver application.
pub(in crate::production::type_semantics) struct SlotContracts<'a> {
    metadata: SharedTypeMetadataV1<'a>,
    dependencies: Vec<SharedTypeMetadataV1<'a>>,
    selections: &'a CanonicalInheritanceSourceSlotSelectionsV1,
    roots: BTreeMap<PersistentDispatchSlotId, Declaration>,
}

impl<'a> SlotContracts<'a> {
    pub(in crate::production::type_semantics) fn new(
        metadata: SharedTypeMetadataV1<'a>,
        dependencies: &[SharedTypeMetadataV1<'a>],
        inventory: &CanonicalSourceInheritanceInventoriesV1,
        selections: &'a CanonicalInheritanceSourceSlotSelectionsV1,
    ) -> Result<Self, Error> {
        let mut roots = BTreeMap::new();
        for slot in inventory
            .records()
            .iter()
            .flat_map(|record| record.slot_schemas().records())
            .flat_map(|schema| schema.slots())
        {
            if roots.contains_key(slot) {
                continue;
            }
            let key = metadata
                .identities
                .canonical_key::<_, DispatchSlotKey>(*slot)
                .map_err(invalid)?;
            let declaration = match (key.owner(), key.role()) {
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
                _ => return Err(invalid("dispatch slot role does not match its declaration")),
            };
            roots.insert(*slot, declaration);
        }
        Ok(Self {
            metadata,
            dependencies: dependencies.to_vec(),
            selections,
            roots,
        })
    }

    pub(super) fn project(
        &self,
        owner: PersistentExactTypeId,
        schemas: &CanonicalInheritanceSlotSchemasV1,
    ) -> Result<CanonicalInheritanceSlotContractsV1, Error> {
        let mut contracts = BTreeMap::new();
        for schema in schemas.records() {
            for slot in schema.slots() {
                let declaration = *self
                    .roots
                    .get(slot)
                    .ok_or_else(|| invalid("dispatch slot has no source root declaration"))?;
                let source_root = match schema.role() {
                    InheritanceSlotSchemaRoleV1::ClassVtable => owner,
                    InheritanceSlotSchemaRoleV1::Interface { interface_exact } => interface_exact,
                };
                let source = self.target(source_root, declaration)?;
                let selection = self
                    .selections
                    .get(owner, schema.role(), *slot)
                    .ok_or_else(|| {
                        invalid("dispatch slot has no resolved implementation selection")
                    })?;
                let implementation =
                    match selection.selection() {
                        InheritanceSourceSlotSelectionV1::Abstract(target) => {
                            InheritanceSlotImplementationV1::Abstract(self.selected_target(
                                selection.receiver(),
                                target,
                                &source,
                            )?)
                        }
                        InheritanceSourceSlotSelectionV1::Concrete(target) => {
                            InheritanceSlotImplementationV1::Concrete(self.selected_target(
                                selection.receiver(),
                                target,
                                &source,
                            )?)
                        }
                        InheritanceSourceSlotSelectionV1::InterfaceDefault(target) => {
                            InheritanceSlotImplementationV1::InterfaceDefault(
                                self.selected_target(selection.receiver(), target, &source)?,
                            )
                        }
                    };
                let contract = InheritanceSlotContractV1::try_new(
                    schema.role(),
                    *slot,
                    declaration,
                    source.signature().clone(),
                    implementation,
                    source.declaration_access().clone(),
                )
                .map_err(invalid)?;
                contracts.insert(contract.key(), contract);
            }
        }
        CanonicalInheritanceSlotContractsV1::try_new(contracts.into_values().collect())
            .map_err(invalid)
    }

    fn target(
        &self,
        root: PersistentExactTypeId,
        declaration: Declaration,
    ) -> Result<InheritanceSlotTargetV1, Error> {
        let origin = declaration
            .origin()
            .ok_or_else(|| invalid("a generated implementation cannot declare a dispatch slot"))?;
        let (metadata, source) = std::iter::once(self.metadata)
            .chain(self.dependencies.iter().copied())
            .find_map(|metadata| {
                metadata
                    .public
                    .callable_interfaces()
                    .declaration(origin)
                    .map(|source| (metadata, source))
            })
            .ok_or_else(|| invalid("dispatch declaration has no callable contract"))?;
        let owner = source
            .owner()
            .nominal_owner()
            .ok_or_else(|| invalid("dispatch declaration has no nominal owner"))?;
        let receiver = self
            .metadata
            .applied_member_receiver(root, owner, &self.dependencies)
            .map_err(invalid)?;
        Ok(InheritanceSlotTargetV1::new(
            declaration,
            self.metadata
                .applied_member_signature(receiver, source)
                .map_err(invalid)?,
            source.modality(),
            metadata
                .callable_declaration_access(source)
                .map_err(invalid)?,
        ))
    }
}
