use std::collections::BTreeMap;

use scoop_identity::{
    DispatchDeclarationOwner, DispatchRole, ExactTypeKey, PersistentDispatchSlotId,
    PersistentTypeId,
};

use super::source_errors::invalid;
use super::*;

type Declaration = InheritanceCallableDeclarationV1;

/// Joins resolved source members and implementation choices without changing
/// slot identity, root visibility, or the schema's declaration order.
pub(in crate::production::type_semantics) struct SlotContracts<'a> {
    sources: BTreeMap<Declaration, DispatchSource>,
    selections: &'a CanonicalInheritanceSourceSlotSelectionsV1,
    roots: BTreeMap<PersistentDispatchSlotId, Declaration>,
}

struct DispatchSource {
    owner: PersistentTypeId,
    callable: InheritanceSourceCallableV1,
    lookup: PersistentSlotContractDomainV1,
}

impl<'a> SlotContracts<'a> {
    pub(in crate::production::type_semantics) fn new(
        export: &ExportHir,
        dependencies: &[SharedTypeMetadataV1<'_>],
        inventory: &CanonicalSourceInheritanceInventoriesV1,
        selections: &'a CanonicalInheritanceSourceSlotSelectionsV1,
    ) -> Result<Self, Error> {
        let mut roots = BTreeMap::new();
        let records = export.dispatch_slot_identities.records().chain(
            export
                .types
                .iter()
                .filter_map(|(_, ty)| match ty {
                    Type::ImportedInterface(interface) => {
                        Some(interface.methods.iter().map(|method| &method.slot))
                    }
                    _ => None,
                })
                .flatten(),
        );
        for record in records {
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
                _ => return Err(invalid("dispatch slot role does not match its declaration")),
            };
            roots.insert(record.id(), declaration);
        }
        let mut required = std::collections::BTreeSet::new();
        for slot in inventory
            .records()
            .iter()
            .flat_map(|record| record.slot_schemas().records())
            .flat_map(|schema| schema.slots())
        {
            required.insert(
                *roots
                    .get(slot)
                    .ok_or_else(|| invalid("dispatch slot has no declaration"))?,
            );
        }
        for selection in selections.records() {
            match selection.selection() {
                InheritanceSourceSlotSelectionV1::Concrete(target)
                | InheritanceSourceSlotSelectionV1::InterfaceDefault(target) => {
                    required.insert(target);
                }
                InheritanceSourceSlotSelectionV1::Abstract => {}
            }
        }
        let mut sources = BTreeMap::new();
        for (id, function) in export.functions.iter() {
            let Some(declaration) = source_callables::identity(export, id) else {
                continue;
            };
            if !required.remove(&declaration) {
                continue;
            }
            let method = function.method.expect("dispatch targets are methods");
            let identity = export.type_identities[method.owner].exact().ok_or(
                Error::MissingExactIdentity {
                    context: "dispatch receiver",
                },
            )?;
            let ExactTypeKey::Nominal(owner) = identity.key() else {
                return Err(Error::GenericOdrRequired(identity.id()));
            };
            sources.insert(
                declaration,
                DispatchSource {
                    owner: *owner,
                    callable: source_callables::local(export, id, declaration)?,
                    lookup: PersistentSlotContractDomainV1::new(domain(
                        export,
                        &function.access.lookup.0,
                    )?),
                },
            );
        }
        for declaration in required {
            let origin = match declaration {
                Declaration::Function(id) => scoop_identity::CallableTemplateOrigin::Function(id),
                Declaration::Getter(id) | Declaration::Setter(id) => {
                    scoop_identity::CallableTemplateOrigin::Accessor(id)
                }
            };
            let metadata = dependencies
                .iter()
                .find(|metadata| {
                    metadata
                        .public
                        .callable_interfaces()
                        .declaration(origin)
                        .is_some()
                })
                .ok_or_else(|| invalid("dispatch target has no dependency declaration"))?;
            let (callable, owner) = source_callables::imported(*metadata, declaration)?;
            sources.insert(
                declaration,
                DispatchSource {
                    owner,
                    callable,
                    lookup: PersistentSlotContractDomainV1::new(
                        PersistentAccessDomainV1::universal(),
                    ),
                },
            );
        }
        Ok(Self {
            sources,
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
                if contracts.contains_key(slot) {
                    continue;
                }

                let declaration = *self
                    .roots
                    .get(slot)
                    .ok_or_else(|| invalid("dispatch slot has no source root declaration"))?;
                let source = self.source(declaration)?;

                let selection = self.selections.get(owner, *slot).ok_or_else(|| {
                    invalid("dispatch slot has no resolved implementation selection")
                })?;
                let implementation = match selection {
                    InheritanceSourceSlotSelectionV1::Abstract => {
                        InheritanceSlotImplementationV1::Abstract
                    }
                    InheritanceSourceSlotSelectionV1::Concrete(target) => {
                        InheritanceSlotImplementationV1::Concrete(self.target(target)?)
                    }
                    InheritanceSourceSlotSelectionV1::InterfaceDefault(target) => {
                        InheritanceSlotImplementationV1::InterfaceDefault(self.target(target)?)
                    }
                };
                let contract = InheritanceSlotContractV1::try_new(
                    *slot,
                    source.owner,
                    declaration,
                    source.callable.signature().clone(),
                    source.lookup.clone(),
                    implementation,
                    source.callable.declaration_access().clone(),
                )
                .map_err(invalid)?;
                insert(&mut contracts, *slot, contract)?;
            }
        }

        CanonicalInheritanceSlotContractsV1::try_new(contracts.into_values().collect())
            .map_err(invalid)
    }

    fn source(&self, declaration: Declaration) -> Result<&DispatchSource, Error> {
        self.sources
            .get(&declaration)
            .ok_or_else(|| invalid("dispatch declaration has no callable contract"))
    }

    fn target(&self, declaration: Declaration) -> Result<InheritanceSlotTargetV1, Error> {
        let source = self.source(declaration)?;
        InheritanceSlotTargetV1::try_new(
            declaration,
            source.owner,
            source.callable.signature().clone(),
            source.callable.modality(),
            source.callable.declaration_access().clone(),
        )
        .map_err(invalid)
    }
}

fn insert<K: Ord, V>(map: &mut BTreeMap<K, V>, key: K, value: V) -> Result<(), Error> {
    if map.insert(key, value).is_some() {
        return Err(invalid("duplicate dispatch projection identity"));
    }
    Ok(())
}

fn domain(export: &ExportHir, source: &AccessDomain) -> Result<PersistentAccessDomainV1, Error> {
    if source.is_empty() {
        return Ok(PersistentAccessDomainV1::empty());
    }
    let constraints = source
        .constraints()
        .iter()
        .map(|constraint| match constraint {
            AccessConstraint::Cone(cone) => Ok(PersistentAccessConstraintV1::Cone(*cone)),
            AccessConstraint::File(file) => Ok(PersistentAccessConstraintV1::File(file.clone())),
            AccessConstraint::LexicalOwner(owner) => {
                persistent_owner(export, *owner).map(PersistentAccessConstraintV1::LexicalOwner)
            }
            AccessConstraint::SubclassesOf(class) => {
                let ty = export.class_applications[export.classes[*class].self_application]
                    .canonical_type;
                exact(export, ty).map(PersistentAccessConstraintV1::SubclassesOf)
            }
        })
        .collect::<Result<Vec<_>, Error>>()?;
    PersistentAccessDomainV1::try_from_constraints(constraints).map_err(|error| {
        Error::InvalidTable {
            table: "access-domain",
            reason: error.to_string(),
        }
    })
}

fn persistent_owner(export: &ExportHir, owner: VisibilityOwner) -> Result<SourceNominalId, Error> {
    let identity = match owner {
        VisibilityOwner::Class(id) => &export.nominal_identities[id],
        VisibilityOwner::Interface(id) => &export.nominal_identities[id],
        VisibilityOwner::Struct(id) => &export.nominal_identities[id],
        VisibilityOwner::Enum(id) => &export.nominal_identities[id],
        VisibilityOwner::Object(id) => &export.nominal_identities[id],
    };
    let source = identity.source().ok_or(Error::MissingExactIdentity {
        context: "slot lexical owner",
    })?;
    Ok(match source {
        HirSourceNominalIdentity::Concrete(record) => SourceNominalId::Concrete(record.id()),
        HirSourceNominalIdentity::Generic(record) => SourceNominalId::GenericTemplate(record.id()),
    })
}
